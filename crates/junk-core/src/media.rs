//! Stream / video pipeline — ytdl-inspired:
//! yt-dlp resolves formats → junk multi-conn downloads streams → ffmpeg merges.
//!
//! Mirrors local `ytdl` (adaptive video+audio + ffmpeg) and `ytdl-audio` (yt-dlp -x).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Deserialize;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::download::{download_url, DownloadOptions};
use crate::progress::{Phase, ProgressEvent};
use crate::util::{default_download_dir, sanitize_filename};
use crate::{JunkError, Result};

/// Default video quality height (like ytdl's 720).
pub const DEFAULT_QUALITY: u32 = 720;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaMode {
    /// Best video ≤ quality + best audio → merge to mp4 (ytdl style)
    Video { max_height: u32 },
    /// Extract audio to mp3 (ytdl-audio style)
    Audio,
}

#[derive(Debug, Clone)]
pub struct MediaJob {
    pub url: String,
    pub title: String,
    pub mode: MediaMode,
    pub dest_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct YtDlpJson {
    title: Option<String>,
    ext: Option<String>,
    #[serde(default)]
    requested_downloads: Vec<RequestedDownload>,
    url: Option<String>,
    #[serde(default)]
    urls: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RequestedDownload {
    url: Option<String>,
    #[serde(default)]
    requested_formats: Vec<RequestedFormat>,
    filename: Option<String>,
    ext: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RequestedFormat {
    url: Option<String>,
    vcodec: Option<String>,
    acodec: Option<String>,
    ext: Option<String>,
    height: Option<u32>,
}

/// Hosts that almost always need yt-dlp (not a raw file).
pub fn looks_like_stream_host(url: &str) -> bool {
    let u = url.to_ascii_lowercase();
    const HOSTS: &[&str] = &[
        "youtube.com",
        "youtu.be",
        "youtube-nocookie.com",
        "vimeo.com",
        "twitch.tv",
        "tiktok.com",
        "twitter.com",
        "x.com",
        "reddit.com",
        "redd.it",
        "instagram.com",
        "facebook.com",
        "fb.watch",
        "bilibili.com",
        "soundcloud.com",
        "bandcamp.com",
        "dailymotion.com",
        "nicovideo.jp",
        "streamable.com",
        "rumble.com",
        "odysee.com",
        "music.youtube.com",
    ];
    HOSTS.iter().any(|h| u.contains(h))
}

pub fn find_yt_dlp() -> Option<PathBuf> {
    which("yt-dlp").or_else(|| which("youtube-dl"))
}

pub fn find_ffmpeg() -> Option<PathBuf> {
    which("ffmpeg")
}

fn which(name: &str) -> Option<PathBuf> {
    if let Ok(p) = std::env::var(format!("JUNK_{}_PATH", name.to_ascii_uppercase().replace('-', "_")))
    {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    std::env::var_os("PATH").and_then(|paths| {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
            #[cfg(windows)]
            {
                let exe = dir.join(format!("{name}.exe"));
                if exe.is_file() {
                    return Some(exe);
                }
            }
        }
        None
    })
}

pub fn default_video_dir() -> PathBuf {
    dirs::video_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Videos")))
        .unwrap_or_else(default_download_dir)
}

pub fn default_music_dir() -> PathBuf {
    dirs::audio_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Music")))
        .unwrap_or_else(default_download_dir)
}

/// Resolve stream URLs with yt-dlp (no download yet).
pub async fn resolve_media(url: &str, mode: MediaMode) -> Result<ResolvedMedia> {
    let ytdlp = find_yt_dlp().ok_or_else(|| {
        JunkError::Other(
            "yt-dlp not found — install yt-dlp for streaming sites (YouTube, etc.)".into(),
        )
    })?;

    let format = match mode {
        MediaMode::Video { max_height } => format!(
            "bestvideo[height<={h}]+bestaudio/best[height<={h}]/best",
            h = max_height
        ),
        MediaMode::Audio => "bestaudio/best".into(),
    };

    let output = Command::new(&ytdlp)
        .args([
            "-f",
            &format,
            "--no-playlist",
            "--no-warnings",
            "-J",
            "--",
            url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| JunkError::Other(format!("yt-dlp spawn failed: {e}")))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(JunkError::Other(format!(
            "yt-dlp failed: {}",
            err.trim().lines().last().unwrap_or("unknown error")
        )));
    }

    let meta: YtDlpJson = serde_json::from_slice(&output.stdout)
        .map_err(|e| JunkError::Other(format!("yt-dlp JSON parse: {e}")))?;

    let title = meta
        .title
        .clone()
        .unwrap_or_else(|| "download".into());
    let safe = sanitize_filename(&title);

    // Collect stream URLs from requested formats / downloads
    let mut streams: Vec<StreamPart> = Vec::new();

    for rd in &meta.requested_downloads {
        if !rd.requested_formats.is_empty() {
            for fmt in &rd.requested_formats {
                if let Some(u) = &fmt.url {
                    let kind = classify_format(fmt.vcodec.as_deref(), fmt.acodec.as_deref());
                    streams.push(StreamPart {
                        url: u.clone(),
                        kind,
                        ext: fmt.ext.clone().unwrap_or_else(|| "bin".into()),
                    });
                }
            }
        } else if let Some(u) = &rd.url {
            streams.push(StreamPart {
                url: u.clone(),
                kind: StreamKind::Combined,
                ext: rd.ext.clone().unwrap_or_else(|| "mp4".into()),
            });
        }
    }

    // Fallback: top-level url
    if streams.is_empty() {
        if let Some(u) = &meta.url {
            streams.push(StreamPart {
                url: u.clone(),
                kind: StreamKind::Combined,
                ext: meta.ext.clone().unwrap_or_else(|| "mp4".into()),
            });
        }
    }

    // Fallback: -g style via second invocation
    if streams.is_empty() {
        streams = resolve_urls_g(&ytdlp, url, &format).await?;
    }

    if streams.is_empty() {
        return Err(JunkError::Other(
            "yt-dlp returned no stream URLs for this link".into(),
        ));
    }

    Ok(ResolvedMedia {
        title: safe,
        streams,
        mode,
    })
}

async fn resolve_urls_g(ytdlp: &Path, url: &str, format: &str) -> Result<Vec<StreamPart>> {
    let output = Command::new(ytdlp)
        .args(["-f", format, "--no-playlist", "-g", "--", url])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| JunkError::Other(format!("yt-dlp -g failed: {e}")))?;

    if !output.status.success() {
        return Ok(vec![]);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let mut parts = Vec::new();
    match lines.len() {
        0 => {}
        1 => parts.push(StreamPart {
            url: lines[0].into(),
            kind: StreamKind::Combined,
            ext: "mp4".into(),
        }),
        _ => {
            parts.push(StreamPart {
                url: lines[0].into(),
                kind: StreamKind::Video,
                ext: "mp4".into(),
            });
            parts.push(StreamPart {
                url: lines[1].into(),
                kind: StreamKind::Audio,
                ext: "m4a".into(),
            });
        }
    }
    Ok(parts)
}

fn classify_format(vcodec: Option<&str>, acodec: Option<&str>) -> StreamKind {
    let v = vcodec.unwrap_or("none");
    let a = acodec.unwrap_or("none");
    let has_v = v != "none" && !v.is_empty();
    let has_a = a != "none" && !a.is_empty();
    match (has_v, has_a) {
        (true, true) => StreamKind::Combined,
        (true, false) => StreamKind::Video,
        (false, true) => StreamKind::Audio,
        _ => StreamKind::Combined,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Video,
    Audio,
    Combined,
}

#[derive(Debug, Clone)]
pub struct StreamPart {
    pub url: String,
    pub kind: StreamKind,
    pub ext: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedMedia {
    pub title: String,
    pub streams: Vec<StreamPart>,
    pub mode: MediaMode,
}

/// Download resolved streams with multi-conn junk engine, then ffmpeg as needed.
pub async fn download_media(
    url: &str,
    mode: MediaMode,
    dest_dir: &Path,
    connections: u32,
    cancel: Arc<AtomicBool>,
    progress: mpsc::Sender<ProgressEvent>,
    job_id: u64,
) -> Result<PathBuf> {
    tokio::fs::create_dir_all(dest_dir).await?;

    let _ = progress
        .send(ProgressEvent {
            job_id,
            filename: "resolving with yt-dlp…".into(),
            bytes_done: 0,
            bytes_total: 0,
            bytes_per_sec: 0.0,
            connections_active: 0,
            eta_secs: None,
            phase: Phase::Connecting,
            error: None,
        })
        .await;

    let resolved = resolve_media(url, mode).await?;
    let title = resolved.title.clone();

    // Audio-only via yt-dlp extract is simpler/reliable for mp3
    if matches!(mode, MediaMode::Audio) {
        return download_audio_ytdlp(url, dest_dir, &title, cancel, progress, job_id).await;
    }

    if find_ffmpeg().is_none() && resolved.streams.len() > 1 {
        return Err(JunkError::Other(
            "ffmpeg required to merge video+audio (like ytdl). Install ffmpeg.".into(),
        ));
    }

    let n = resolved.streams.len().max(1) as u64;
    let mut part_paths: Vec<(StreamKind, PathBuf)> = Vec::new();

    for (i, stream) in resolved.streams.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(JunkError::Cancelled);
        }
        let suffix = match stream.kind {
            StreamKind::Video => "video",
            StreamKind::Audio => "audio",
            StreamKind::Combined => "media",
        };
        let part_name = format!(".junk_{}_{}.{}", std::process::id(), suffix, stream.ext);
        let part_path = dest_dir.join(&part_name);

        let (tx, mut rx) = mpsc::channel::<ProgressEvent>(64);
        let opts = DownloadOptions {
            connections,
            cancel: Arc::clone(&cancel),
            pause: Arc::new(AtomicBool::new(false)),
            job_id,
        };

        let url_s = stream.url.clone();
        let dest = part_path.clone();
        let dl = tokio::spawn(async move { download_url(&url_s, &dest, opts, tx).await });

        let progress_c = progress.clone();
        let title_c = title.clone();
        let idx = i as u64;
        let pump = tokio::spawn(async move {
            while let Some(mut ev) = rx.recv().await {
                // Map partial progress into overall band
                if ev.bytes_total > 0 {
                    let local = ev.bytes_done as f64 / ev.bytes_total as f64;
                    let overall = (idx as f64 + local) / n as f64;
                    ev.filename = format!("{title_c} [{}/{}]", idx + 1, n);
                    // Synthetic totals for UI: 10000 units
                    ev.bytes_done = (overall * 10_000.0) as u64;
                    ev.bytes_total = 10_000;
                }
                let _ = progress_c.send(ev).await;
            }
        });

        let path = dl
            .await
            .map_err(|e| JunkError::Other(e.to_string()))??;
        let _ = pump.await;
        part_paths.push((stream.kind, path));
    }

    // Single combined stream — just rename into place
    if part_paths.len() == 1 {
        let (_, path) = part_paths.remove(0);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mp4");
        let final_path = unique_media_path(dest_dir, &title, ext);
        tokio::fs::rename(&path, &final_path).await?;
        emit_done(&progress, job_id, &final_path).await;
        return Ok(final_path);
    }

    // Merge video + audio with ffmpeg (ytdl path)
    let video = part_paths
        .iter()
        .find(|(k, _)| *k == StreamKind::Video)
        .map(|(_, p)| p.clone());
    let audio = part_paths
        .iter()
        .find(|(k, _)| *k == StreamKind::Audio)
        .map(|(_, p)| p.clone());

    let (Some(vpath), Some(apath)) = (video, audio) else {
        // odd case: treat first as output
        let (_, path) = part_paths.remove(0);
        let final_path = unique_media_path(dest_dir, &title, "mp4");
        tokio::fs::rename(&path, &final_path).await?;
        emit_done(&progress, job_id, &final_path).await;
        return Ok(final_path);
    };

    let _ = progress
        .send(ProgressEvent {
            job_id,
            filename: format!("ffmpeg merge — {title}"),
            bytes_done: 9500,
            bytes_total: 10_000,
            bytes_per_sec: 0.0,
            connections_active: 0,
            eta_secs: None,
            phase: Phase::Finalizing,
            error: None,
        })
        .await;

    let final_path = unique_media_path(dest_dir, &title, "mp4");
    ffmpeg_merge(&vpath, &apath, &final_path).await?;

    let _ = tokio::fs::remove_file(&vpath).await;
    let _ = tokio::fs::remove_file(&apath).await;

    emit_done(&progress, job_id, &final_path).await;
    Ok(final_path)
}

async fn download_audio_ytdlp(
    url: &str,
    dest_dir: &Path,
    title: &str,
    cancel: Arc<AtomicBool>,
    progress: mpsc::Sender<ProgressEvent>,
    job_id: u64,
) -> Result<PathBuf> {
    let ytdlp = find_yt_dlp().ok_or_else(|| JunkError::Other("yt-dlp not found".into()))?;
    if find_ffmpeg().is_none() {
        return Err(JunkError::Other(
            "ffmpeg required for audio extract (mp3)".into(),
        ));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(JunkError::Cancelled);
    }

    let _ = progress
        .send(ProgressEvent {
            job_id,
            filename: format!("yt-dlp audio — {title}"),
            bytes_done: 1000,
            bytes_total: 10_000,
            bytes_per_sec: 0.0,
            connections_active: 1,
            eta_secs: None,
            phase: Phase::Downloading,
            error: None,
        })
        .await;

    let out_template = dest_dir.join("%(title)s.%(ext)s");
    let output = Command::new(&ytdlp)
        .args([
            "-x",
            "--audio-format",
            "mp3",
            "--audio-quality",
            "0",
            "--embed-metadata",
            "--no-playlist",
            "-o",
            out_template.to_str().unwrap_or("%(title)s.%(ext)s"),
            "--",
            url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| JunkError::Other(format!("yt-dlp audio: {e}")))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(JunkError::Other(format!(
            "yt-dlp audio failed: {}",
            err.trim().lines().last().unwrap_or("error")
        )));
    }

    // Best-effort find newest mp3 in dest
    let final_path = newest_with_ext(dest_dir, "mp3")
        .unwrap_or_else(|| dest_dir.join(format!("{title}.mp3")));
    emit_done(&progress, job_id, &final_path).await;
    Ok(final_path)
}

async fn ffmpeg_merge(video: &Path, audio: &Path, out: &Path) -> Result<()> {
    let ffmpeg = find_ffmpeg().ok_or_else(|| JunkError::Other("ffmpeg not found".into()))?;
    let output = Command::new(&ffmpeg)
        .args([
            "-y",
            "-i",
            video.to_str().unwrap_or(""),
            "-i",
            audio.to_str().unwrap_or(""),
            "-c:v",
            "copy",
            "-c:a",
            "aac",
            "-strict",
            "experimental",
            out.to_str().unwrap_or("out.mp4"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| JunkError::Other(format!("ffmpeg spawn: {e}")))?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let tail: String = err.chars().rev().take(800).collect::<String>().chars().rev().collect();
        return Err(JunkError::Other(format!("ffmpeg merge failed: {tail}")));
    }
    Ok(())
}

fn unique_media_path(dir: &Path, title: &str, ext: &str) -> PathBuf {
    let base = dir.join(format!("{title}.{ext}"));
    if !base.exists() {
        return base;
    }
    for n in 1..1000 {
        let p = dir.join(format!("{title}-{n}.{ext}"));
        if !p.exists() {
            return p;
        }
    }
    dir.join(format!("{title}-dup.{ext}"))
}

fn newest_with_ext(dir: &Path, ext: &str) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    let rd = std::fs::read_dir(dir).ok()?;
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some(ext) {
            continue;
        }
        let mt = e.metadata().ok()?.modified().ok()?;
        if best.as_ref().map(|(t, _)| mt > *t).unwrap_or(true) {
            best = Some((mt, p));
        }
    }
    best.map(|(_, p)| p)
}

async fn emit_done(progress: &mpsc::Sender<ProgressEvent>, job_id: u64, path: &Path) {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("done")
        .to_string();
    let _ = progress
        .send(ProgressEvent {
            job_id,
            filename: name,
            bytes_done: 10_000,
            bytes_total: 10_000,
            bytes_per_sec: 0.0,
            connections_active: 0,
            eta_secs: Some(0),
            phase: Phase::Done,
            error: None,
        })
        .await;
}

/// Decide if this URL should go through the media pipeline.
pub fn should_use_media(url: &str, force: bool, force_http: bool) -> bool {
    if force_http {
        return false;
    }
    if force {
        return true;
    }
    looks_like_stream_host(url)
}
