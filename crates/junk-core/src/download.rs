use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use futures::StreamExt;
use reqwest::header::{
    ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, ETAG, LAST_MODIFIED, RANGE,
};
use reqwest::{Client, StatusCode};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{mpsc, Mutex, Semaphore};

use crate::progress::{Phase, ProgressEvent};
use crate::resume::{
    clear_state, load_state, part_path, save_state, split_segments, ResumeState, SegmentState,
};
use crate::util::{filename_from_url, sanitize_filename, unique_path};
use crate::{JunkError, Result};

const USER_AGENT: &str = "junk/0.1 (multi-conn; +https://github.com/junk)";
const MAX_RETRIES: u32 = 4;
const PROGRESS_MIN_MS: u128 = 50;

#[derive(Clone)]
pub struct DownloadOptions {
    pub connections: u32,
    pub cancel: Arc<AtomicBool>,
    pub pause: Arc<AtomicBool>,
    pub job_id: u64,
}

struct Probe {
    final_url: String,
    size: Option<u64>,
    accept_ranges: bool,
    etag: Option<String>,
    last_modified: Option<String>,
    filename: Option<String>,
}

pub async fn download_url(
    url: &str,
    dest: &Path,
    opts: DownloadOptions,
    progress: mpsc::Sender<ProgressEvent>,
) -> Result<PathBuf> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let client = Client::builder()
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::limited(10))
        .connect_timeout(std::time::Duration::from_secs(30))
        .pool_max_idle_per_host(opts.connections as usize)
        .build()
        .map_err(|e| JunkError::Http(e.to_string()))?;

    emit(
        &progress,
        ProgressEvent {
            job_id: opts.job_id,
            filename: dest
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("download")
                .to_string(),
            bytes_done: 0,
            bytes_total: 0,
            bytes_per_sec: 0.0,
            connections_active: 0,
            eta_secs: None,
            phase: Phase::Connecting,
            error: None,
        },
    )
    .await;

    check_cancel(&opts)?;

    let probe = probe_url(&client, url).await?;
    let mut final_dest = dest.to_path_buf();

    if let Some(name) = &probe.filename {
        if let Some(parent) = final_dest.parent() {
            final_dest = parent.join(name);
        }
    }

    // Resume only if part+state exist for this dest basename
    let mut state_opt = load_state(&final_dest)?;
    if let Some(ref st) = state_opt {
        if st.url != probe.final_url && st.url != url {
            state_opt = None;
        }
        if let (Some(sz), Some(st)) = (probe.size, state_opt.as_ref()) {
            if st.total_size != sz {
                state_opt = None;
            }
        }
        if let (Some(etag), Some(st)) = (&probe.etag, state_opt.as_ref()) {
            if st.etag.as_ref() != Some(etag) {
                // etag mismatch — fresh download unless we have no etag stored
                if st.etag.is_some() {
                    state_opt = None;
                }
            }
        }
    }

    let use_multi = probe.accept_ranges && probe.size.is_some_and(|s| s > 0);

    let result = if use_multi {
        let total = probe.size.unwrap();
        multi_download(
            &client,
            &probe.final_url,
            &final_dest,
            total,
            &probe,
            state_opt,
            &opts,
            &progress,
        )
        .await
    } else {
        single_download(
            &client,
            &probe.final_url,
            &final_dest,
            probe.size,
            &opts,
            &progress,
        )
        .await
    };

    match result {
        Ok(path) => {
            emit(
                &progress,
                ProgressEvent {
                    job_id: opts.job_id,
                    filename: path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("download")
                        .to_string(),
                    bytes_done: probe.size.unwrap_or(0),
                    bytes_total: probe.size.unwrap_or(0),
                    bytes_per_sec: 0.0,
                    connections_active: 0,
                    eta_secs: Some(0),
                    phase: Phase::Done,
                    error: None,
                },
            )
            .await;
            Ok(path)
        }
        Err(e) => {
            let msg = e.to_string();
            emit(
                &progress,
                ProgressEvent {
                    job_id: opts.job_id,
                    filename: final_dest
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("download")
                        .to_string(),
                    bytes_done: 0,
                    bytes_total: probe.size.unwrap_or(0),
                    bytes_per_sec: 0.0,
                    connections_active: 0,
                    eta_secs: None,
                    phase: Phase::Error,
                    error: Some(msg),
                },
            )
            .await;
            Err(e)
        }
    }
}

async fn emit(tx: &mpsc::Sender<ProgressEvent>, ev: ProgressEvent) {
    let _ = tx.send(ev).await;
}

fn check_cancel(opts: &DownloadOptions) -> Result<()> {
    if opts.cancel.load(Ordering::Relaxed) {
        Err(JunkError::Cancelled)
    } else {
        Ok(())
    }
}

async fn wait_if_paused(opts: &DownloadOptions) -> Result<()> {
    while opts.pause.load(Ordering::Relaxed) {
        check_cancel(opts)?;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Ok(())
}

async fn probe_url(client: &Client, url: &str) -> Result<Probe> {
    // Prefer HEAD
    let head = client.head(url).send().await;
    if let Ok(resp) = head {
        if resp.status().is_success() {
            return probe_from_response(resp, url).await;
        }
    }

    // Fallback: ranged GET 0-0
    let resp = client
        .get(url)
        .header(RANGE, "bytes=0-0")
        .send()
        .await
        .map_err(|e| JunkError::Http(e.to_string()))?;

    if resp.status() == StatusCode::PARTIAL_CONTENT || resp.status().is_success() {
        return probe_from_response(resp, url).await;
    }

    Err(JunkError::Http(format!(
        "probe failed: HTTP {}",
        resp.status()
    )))
}

async fn probe_from_response(resp: reqwest::Response, original_url: &str) -> Result<Probe> {
    let status = resp.status();
    let final_url = resp.url().to_string();
    let headers = resp.headers().clone();

    let etag = headers
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let last_modified = headers
        .get(LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let accept_ranges = headers
        .get(ACCEPT_RANGES)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_ascii_lowercase().contains("bytes"))
        .unwrap_or(false)
        || status == StatusCode::PARTIAL_CONTENT
        || headers.get(CONTENT_RANGE).is_some();

    let mut size = headers
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    if let Some(cr) = headers.get(CONTENT_RANGE).and_then(|v| v.to_str().ok()) {
        // bytes 0-0/12345
        if let Some(total) = cr.split('/').nth(1) {
            if total != "*" {
                if let Ok(t) = total.parse::<u64>() {
                    size = Some(t);
                }
            }
        }
        // PARTIAL means ranges work even if Accept-Ranges missing
    }

    let filename = headers
        .get(CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_content_disposition)
        .or_else(|| {
            let n = filename_from_url(&final_url);
            if n != "download.bin" {
                Some(n)
            } else {
                let n = filename_from_url(original_url);
                if n != "download.bin" {
                    Some(n)
                } else {
                    None
                }
            }
        });

    // Consume body if any (0-0 or empty HEAD)
    let _ = resp.bytes().await;

    Ok(Probe {
        final_url,
        size,
        accept_ranges,
        etag,
        last_modified,
        filename,
    })
}

fn parse_content_disposition(h: &str) -> Option<String> {
    // filename="foo" or filename*=UTF-8''foo
    for part in h.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename*=") {
            let rest = rest.trim_matches('"');
            if let Some(idx) = rest.find("''") {
                return Some(sanitize_filename(&percent_simple(&rest[idx + 2..])));
            }
            return Some(sanitize_filename(rest.trim_matches('"')));
        }
        if let Some(rest) = part.strip_prefix("filename=") {
            return Some(sanitize_filename(rest.trim_matches('"')));
        }
    }
    None
}

fn percent_simple(s: &str) -> String {
    // reuse util via filename path — simple replace
    s.replace("%20", " ")
}

#[allow(clippy::too_many_arguments)]
async fn multi_download(
    client: &Client,
    url: &str,
    dest: &Path,
    total: u64,
    probe: &Probe,
    existing: Option<ResumeState>,
    opts: &DownloadOptions,
    progress: &mpsc::Sender<ProgressEvent>,
) -> Result<PathBuf> {
    let part = part_path(dest);
    let segments: Vec<SegmentState> = if let Some(st) = existing {
        if st.total_size == total && part.exists() {
            st.segments
        } else {
            let _ = tokio::fs::remove_file(&part).await;
            split_segments(total, opts.connections)
        }
    } else {
        if part.exists() {
            let _ = tokio::fs::remove_file(&part).await;
        }
        split_segments(total, opts.connections)
    };

    // Preallocate
    {
        let f = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&part)
            .await?;
        f.set_len(total).await?;
    }

    let done_total = Arc::new(AtomicU64::new(
        segments.iter().map(|s| s.done.min(s.len())).sum(),
    ));
    let active = Arc::new(AtomicU64::new(0));
    let start = Instant::now();
    let segments = Arc::new(Mutex::new(segments));
    let filename = dest
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("download")
        .to_string();

    // Progress reporter task
    let cancel_flag = Arc::clone(&opts.cancel);
    let prog_tx = progress.clone();
    let done_c = Arc::clone(&done_total);
    let active_c = Arc::clone(&active);
    let fname = filename.clone();
    let job_id = opts.job_id;
    let reporter = tokio::spawn(async move {
        while !cancel_flag.load(Ordering::Relaxed) {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let d = done_c.load(Ordering::Relaxed);
            if d >= total {
                break;
            }
            let elapsed = start.elapsed().as_secs_f64().max(0.001);
            let rate = d as f64 / elapsed;
            let left = total.saturating_sub(d);
            let _ = prog_tx
                .send(ProgressEvent {
                    job_id,
                    filename: fname.clone(),
                    bytes_done: d,
                    bytes_total: total,
                    bytes_per_sec: rate,
                    connections_active: active_c.load(Ordering::Relaxed) as u32,
                    eta_secs: ProgressEvent::eta(left, rate),
                    phase: Phase::Downloading,
                    error: None,
                })
                .await;
        }
    });

    let sem = Arc::new(Semaphore::new(opts.connections as usize));
    let mut handles = Vec::new();
    let n_segs = {
        let s = segments.lock().await;
        s.len()
    };

    for idx in 0..n_segs {
        let permit = sem.clone().acquire_owned().await.unwrap();
        let client = client.clone();
        let url = url.to_string();
        let part = part.clone();
        let segments = Arc::clone(&segments);
        let done_total = Arc::clone(&done_total);
        let active = Arc::clone(&active);
        let opts = opts.clone();
        let dest_for_state = dest.to_path_buf();
        let probe_url = probe.final_url.clone();
        let etag = probe.etag.clone();
        let lm = probe.last_modified.clone();

        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let mut attempt = 0;
            loop {
                let (start_byte, end_byte, already) = {
                    let segs = segments.lock().await;
                    let s = &segs[idx];
                    if s.is_complete() {
                        return Ok::<(), JunkError>(());
                    }
                    (s.start, s.end, s.done)
                };

                attempt += 1;
                match download_segment(
                    &client,
                    &url,
                    &part,
                    start_byte,
                    end_byte,
                    already,
                    idx,
                    &segments,
                    &done_total,
                    &active,
                    &opts,
                )
                .await
                {
                    Ok(()) => {
                        let segs = segments.lock().await.clone();
                        let st = ResumeState {
                            url: probe_url.clone(),
                            total_size: total,
                            etag: etag.clone(),
                            last_modified: lm.clone(),
                            segments: segs,
                        };
                        let _ = save_state(&dest_for_state, &st);
                        return Ok(());
                    }
                    Err(JunkError::Cancelled) => return Err(JunkError::Cancelled),
                    Err(e) if attempt < MAX_RETRIES => {
                        tokio::time::sleep(std::time::Duration::from_millis(
                            200 * attempt as u64,
                        ))
                        .await;
                        let _ = e;
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
        }));
    }

    let mut first_err: Option<JunkError> = None;
    for h in handles {
        match h.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                opts.cancel.store(true, Ordering::Relaxed);
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
            Err(e) => {
                opts.cancel.store(true, Ordering::Relaxed);
                if first_err.is_none() {
                    first_err = Some(JunkError::Other(e.to_string()));
                }
            }
        }
    }

    reporter.abort();

    if let Some(e) = first_err {
        // save state for resume
        let segs = segments.lock().await.clone();
        let st = ResumeState {
            url: probe.final_url.clone(),
            total_size: total,
            etag: probe.etag.clone(),
            last_modified: probe.last_modified.clone(),
            segments: segs,
        };
        let _ = save_state(dest, &st);
        return Err(e);
    }

    check_cancel(opts)?;

    emit(
        progress,
        ProgressEvent {
            job_id: opts.job_id,
            filename: filename.clone(),
            bytes_done: total,
            bytes_total: total,
            bytes_per_sec: 0.0,
            connections_active: 0,
            eta_secs: Some(0),
            phase: Phase::Finalizing,
            error: None,
        },
    )
    .await;

    // Finalize: unique path if dest exists
    let final_path = if dest.exists() {
        unique_path(dest)
    } else {
        dest.to_path_buf()
    };

    tokio::fs::rename(&part, &final_path).await?;
    clear_state(dest);
    if final_path != dest {
        clear_state(&final_path);
    }

    // best-effort fsync
    if let Ok(f) = std::fs::File::open(&final_path) {
        let _ = f.sync_all();
    }

    Ok(final_path)
}

#[allow(clippy::too_many_arguments)]
async fn download_segment(
    client: &Client,
    url: &str,
    part: &Path,
    seg_start: u64,
    seg_end: u64,
    already: u64,
    idx: usize,
    segments: &Arc<Mutex<Vec<SegmentState>>>,
    done_total: &Arc<AtomicU64>,
    active: &Arc<AtomicU64>,
    opts: &DownloadOptions,
) -> Result<()> {
    let mut written = already;
    let abs_start = seg_start + written;
    if abs_start > seg_end {
        return Ok(());
    }

    active.fetch_add(1, Ordering::Relaxed);
    let result = async {
        wait_if_paused(opts).await?;
        check_cancel(opts)?;

        let range = format!("bytes={abs_start}-{seg_end}");
        let resp = client
            .get(url)
            .header(RANGE, range)
            .send()
            .await
            .map_err(|e| JunkError::Http(e.to_string()))?;

        if !(resp.status() == StatusCode::PARTIAL_CONTENT || resp.status().is_success()) {
            return Err(JunkError::Http(format!(
                "segment {idx}: HTTP {}",
                resp.status()
            )));
        }

        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(part)
            .await?;
        file.seek(SeekFrom::Start(abs_start)).await?;

        let need = seg_end - abs_start + 1;
        let mut got = 0u64;
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            wait_if_paused(opts).await?;
            check_cancel(opts)?;
            let chunk = chunk.map_err(|e| JunkError::Http(e.to_string()))?;
            file.write_all(&chunk).await?;
            let n = chunk.len() as u64;
            written += n;
            got += n;
            done_total.fetch_add(n, Ordering::Relaxed);
            {
                let mut segs = segments.lock().await;
                segs[idx].done = written;
            }
            if got >= need {
                break;
            }
        }

        file.flush().await?;
        if written < (seg_end - seg_start + 1) {
            return Err(JunkError::Http(format!(
                "segment {idx}: short read ({written}/{})",
                seg_end - seg_start + 1
            )));
        }
        Ok(())
    }
    .await;

    active.fetch_sub(1, Ordering::Relaxed);
    result
}

async fn single_download(
    client: &Client,
    url: &str,
    dest: &Path,
    known_size: Option<u64>,
    opts: &DownloadOptions,
    progress: &mpsc::Sender<ProgressEvent>,
) -> Result<PathBuf> {
    let part = part_path(dest);
    let mut offset = 0u64;
    if part.exists() {
        if let Ok(meta) = tokio::fs::metadata(&part).await {
            offset = meta.len();
        }
    }

    check_cancel(opts)?;
    wait_if_paused(opts).await?;

    let mut req = client.get(url);
    if offset > 0 {
        req = req.header(RANGE, format!("bytes={offset}-"));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| JunkError::Http(e.to_string()))?;

    let status = resp.status();
    if !(status.is_success() || status == StatusCode::PARTIAL_CONTENT) {
        return Err(JunkError::Http(format!("HTTP {status}")));
    }

    let total = if status == StatusCode::PARTIAL_CONTENT {
        if let Some(cr) = resp.headers().get(CONTENT_RANGE).and_then(|v| v.to_str().ok()) {
            cr.split('/')
                .nth(1)
                .and_then(|t| t.parse().ok())
                .or(known_size)
        } else {
            known_size
        }
    } else {
        // full body — restart if we had offset
        if offset > 0 && status != StatusCode::PARTIAL_CONTENT {
            offset = 0;
            let _ = tokio::fs::remove_file(&part).await;
        }
        resp.headers()
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .map(|l| l + offset)
            .or(known_size)
    };

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(offset == 0)
        .open(&part)
        .await?;
    if offset > 0 {
        file.seek(SeekFrom::End(0)).await?;
    }

    let filename = dest
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("download")
        .to_string();
    let mut done = offset;
    let start = Instant::now();
    let mut last = Instant::now();
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        wait_if_paused(opts).await?;
        check_cancel(opts)?;
        let chunk = chunk.map_err(|e| JunkError::Http(e.to_string()))?;
        file.write_all(&chunk).await?;
        done += chunk.len() as u64;

        if last.elapsed().as_millis() >= PROGRESS_MIN_MS {
            last = Instant::now();
            let elapsed = start.elapsed().as_secs_f64().max(0.001);
            let rate = (done.saturating_sub(offset)) as f64 / elapsed;
            let total_b = total.unwrap_or(0);
            let left = total_b.saturating_sub(done);
            emit(
                progress,
                ProgressEvent {
                    job_id: opts.job_id,
                    filename: filename.clone(),
                    bytes_done: done,
                    bytes_total: total_b,
                    bytes_per_sec: rate,
                    connections_active: 1,
                    eta_secs: if total_b > 0 {
                        ProgressEvent::eta(left, rate)
                    } else {
                        None
                    },
                    phase: Phase::Downloading,
                    error: None,
                },
            )
            .await;
        }
    }

    file.flush().await?;

    emit(
        progress,
        ProgressEvent {
            job_id: opts.job_id,
            filename: filename.clone(),
            bytes_done: done,
            bytes_total: total.unwrap_or(done),
            bytes_per_sec: 0.0,
            connections_active: 0,
            eta_secs: Some(0),
            phase: Phase::Finalizing,
            error: None,
        },
    )
    .await;

    let final_path = if dest.exists() {
        unique_path(dest)
    } else {
        dest.to_path_buf()
    };
    tokio::fs::rename(&part, &final_path).await?;
    clear_state(dest);

    Ok(final_path)
}
