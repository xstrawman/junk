//! Integration tests against a local tiny_http server.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use junk_core::{download_url, DownloadOptions};
use sha2::{Digest, Sha256};
use tiny_http::{Header, Response, Server, StatusCode};
use tokio::sync::mpsc;

fn spawn_range_server(body: &'static [u8]) -> (String, thread::JoinHandle<()>) {
    let server = Server::http("127.0.0.1:0").expect("bind");
    let port = server.server_addr().to_ip().unwrap().port();
    let url = format!("http://127.0.0.1:{port}/file.bin");

    let handle = thread::spawn(move || {
        for request in server.incoming_requests() {
            let method = request.method().as_str().to_string();
            let range = request
                .headers()
                .iter()
                .find(|h| h.field.equiv("Range"))
                .map(|h| h.value.as_str().to_string());

            if method == "HEAD" {
                let mut response = Response::empty(StatusCode(200));
                response.add_header(
                    Header::from_bytes(&b"Content-Length"[..], body.len().to_string().as_bytes())
                        .unwrap(),
                );
                response.add_header(
                    Header::from_bytes(&b"Accept-Ranges"[..], &b"bytes"[..]).unwrap(),
                );
                let _ = request.respond(response);
                continue;
            }

            if let Some(r) = range {
                // bytes=start-end
                let r = r.trim_start_matches("bytes=");
                let mut parts = r.split('-');
                let start: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
                let end: u64 = parts
                    .next()
                    .and_then(|s| if s.is_empty() { None } else { s.parse().ok() })
                    .unwrap_or((body.len() as u64) - 1);
                let end = end.min((body.len() as u64) - 1);
                let start = start.min(end);
                let slice = &body[start as usize..=end as usize];
                let mut response = Response::from_data(slice.to_vec()).with_status_code(StatusCode(206));
                response.add_header(
                    Header::from_bytes(
                        &b"Content-Range"[..],
                        format!("bytes {start}-{end}/{}", body.len()).as_bytes(),
                    )
                    .unwrap(),
                );
                response.add_header(
                    Header::from_bytes(&b"Accept-Ranges"[..], &b"bytes"[..]).unwrap(),
                );
                let _ = request.respond(response);
            } else {
                let mut response = Response::from_data(body.to_vec());
                response.add_header(
                    Header::from_bytes(&b"Content-Length"[..], body.len().to_string().as_bytes())
                        .unwrap(),
                );
                response.add_header(
                    Header::from_bytes(&b"Accept-Ranges"[..], &b"bytes"[..]).unwrap(),
                );
                let _ = request.respond(response);
            }
        }
    });

    // give server a moment
    thread::sleep(Duration::from_millis(50));
    (url, handle)
}

#[tokio::test]
async fn multi_conn_download_matches_sha256() {
    // 256 KiB patterned body
    let body: &'static [u8] = {
        let mut v = vec![0u8; 256 * 1024];
        for (i, b) in v.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        // leak for static lifetime for server thread
        Box::leak(v.into_boxed_slice())
    };

    let expected = {
        let mut h = Sha256::new();
        h.update(body);
        hex::encode(h.finalize())
    };

    let (url, _server) = spawn_range_server(body);
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("file.bin");

    let (tx, mut rx) = mpsc::channel(64);
    let opts = DownloadOptions {
        connections: 8,
        cancel: Arc::new(AtomicBool::new(false)),
        pause: Arc::new(AtomicBool::new(false)),
        job_id: 1,
    };

    let path = download_url(&url, &dest, opts, tx).await.expect("download");
    // drain progress
    while rx.try_recv().is_ok() {}

    let got = std::fs::read(&path).unwrap();
    assert_eq!(got.len(), body.len());
    let mut h = Sha256::new();
    h.update(&got);
    assert_eq!(hex::encode(h.finalize()), expected);
}

#[tokio::test]
async fn cancel_mid_download() {
    // Slow server: sleep per chunk so cancel wins the race.
    let server = Server::http("127.0.0.1:0").expect("bind");
    let port = server.server_addr().to_ip().unwrap().port();
    let url = format!("http://127.0.0.1:{port}/slow.bin");
    let body_len = 8 * 1024 * 1024u64;
    let _server = thread::spawn(move || {
        for request in server.incoming_requests() {
            let method = request.method().as_str().to_string();
            if method == "HEAD" {
                let mut response = Response::empty(StatusCode(200));
                response.add_header(
                    Header::from_bytes(&b"Content-Length"[..], body_len.to_string().as_bytes())
                        .unwrap(),
                );
                response.add_header(
                    Header::from_bytes(&b"Accept-Ranges"[..], &b"bytes"[..]).unwrap(),
                );
                let _ = request.respond(response);
                continue;
            }
            let range = request
                .headers()
                .iter()
                .find(|h| h.field.equiv("Range"))
                .map(|h| h.value.as_str().to_string());
            if let Some(r) = range {
                let r = r.trim_start_matches("bytes=");
                let mut parts = r.split('-');
                let start: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
                let end: u64 = parts
                    .next()
                    .and_then(|s| if s.is_empty() { None } else { s.parse().ok() })
                    .unwrap_or(body_len - 1);
                let end = end.min(body_len - 1);
                let len = (end - start + 1) as usize;
                // Slow full segment body so cancel can fire mid-stream
                thread::sleep(Duration::from_millis(500));
                let data = vec![1u8; len];
                let mut response =
                    Response::from_data(data).with_status_code(StatusCode(206));
                response.add_header(
                    Header::from_bytes(
                        &b"Content-Range"[..],
                        format!("bytes {start}-{end}/{body_len}").as_bytes(),
                    )
                    .unwrap(),
                );
                let _ = request.respond(response);
            } else {
                thread::sleep(Duration::from_millis(500));
                let _ = request.respond(Response::from_data(vec![1u8; body_len as usize]));
            }
        }
    });
    thread::sleep(Duration::from_millis(50));

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("slow.bin");
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_t = Arc::clone(&cancel);
    let (tx, _rx) = mpsc::channel(64);
    let opts = DownloadOptions {
        connections: 4,
        cancel: cancel_t,
        pause: Arc::new(AtomicBool::new(false)),
        job_id: 2,
    };

    let handle = tokio::spawn(async move { download_url(&url, &dest, opts, tx).await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    cancel.store(true, Ordering::Relaxed);
    let res = handle.await.unwrap();
    assert!(res.is_err(), "expected cancel, got {res:?}");
}
