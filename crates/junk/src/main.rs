//! junk — one product: hypersonic multi-conn + streams (aria2 × ytdl × ffmpeg).
//!
//! CLI (default): ASCII art progress, clipboard-first.
//! TUI (`junk tui`): full arcade UI — especially nice on Mac Terminal/iTerm.

mod arcade;
mod ascii;
mod clipboard;
mod tui_app;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use junk_core::{
    default_download_dir, default_music_dir, default_video_dir, download_media, download_url,
    find_ffmpeg, find_ventoy_mounts, find_yt_dlp, human_bytes, should_use_media, DownloadOptions,
    MediaMode, Phase, ProgressEvent, DEFAULT_QUALITY,
};
use tokio::sync::mpsc;

use crate::ascii::{banner, ok, print_syringe, progress_line, stage, warn};
use crate::clipboard::{clipboard_url, looks_like_url, normalize_paste};

#[derive(Parser, Debug)]
#[command(
    name = "junk",
    about = "Hypersonic downloads: multi-conn HTTP + streaming video (yt-dlp × ffmpeg)",
    long_about = "junk = aria2-style multi-conn × ytdl streams × ffmpeg merge.\n\n\
  junk                     # clipboard URL → download (ASCII progress)\n\
  junk <url>               # direct / stream auto-detect\n\
  junk --audio <url>       # MP3 (ytdl-audio style → ~/Music)\n\
  junk --ventoy <iso-url>  # distrohopper: ISO → Ventoy\n\
  junk tui                 # arcade TUI (great on Mac)\n",
    version
)]
struct Cli {
    /// Download directory (files). Streams default to ~/Videos or ~/Music.
    #[arg(short = 'd', long = "dir", global = true)]
    dir: Option<PathBuf>,

    /// Download straight to a detected Ventoy mount
    #[arg(long = "ventoy", global = true)]
    ventoy: bool,

    /// Parallel connections (1–32, default 16)
    #[arg(short = 'c', long = "connections", default_value_t = 16, global = true)]
    connections: u32,

    /// Max video height for streams (default 720, like ytdl)
    #[arg(short = 'q', long = "quality", default_value_t = DEFAULT_QUALITY)]
    quality: u32,

    /// Force audio extract → mp3 in ~/Music
    #[arg(long = "audio", global = true)]
    audio: bool,

    /// Force media pipeline (yt-dlp) even if host looks like a raw file
    #[arg(long = "stream", global = true)]
    stream: bool,

    /// Force plain HTTP multi-conn (skip yt-dlp)
    #[arg(long = "http", global = true)]
    http_only: bool,

    /// Do not auto-read clipboard when no URL given
    #[arg(long = "no-clipboard", global = true)]
    no_clipboard: bool,

    /// Skip the ASCII banner
    #[arg(long = "plain", global = true)]
    plain: bool,

    #[command(subcommand)]
    command: Option<Commands>,

    /// URLs (optional — clipboard used when empty)
    #[arg(trailing_var_arg = true, allow_hyphen_values = false)]
    urls: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Arcade TUI (recommended on Mac Terminal / iTerm)
    Tui,
    /// Distrohopper: multi-conn ISO(s) onto Ventoy
    Ventoy {
        urls: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let connections = cli.connections.clamp(1, 32);

    match cli.command {
        Some(Commands::Tui) => {
            let dir = resolve_file_dir(cli.dir, cli.ventoy)?;
            // TUI shares engine; Mac users land here happily
            tui_app::run(dir, connections).await?;
        }
        Some(Commands::Ventoy { urls }) => {
            let urls = resolve_urls(urls, cli.no_clipboard)?;
            if urls.is_empty() {
                bail!("ventoy mode needs a URL (arg or clipboard)");
            }
            let dir = resolve_ventoy()?;
            if !cli.plain {
                banner();
            }
            stage(&format!("VENTOY ← {}", dir.display()));
            run_jobs(
                urls,
                dir,
                connections,
                cli.quality,
                false,
                true, // http only for ISOs
                false,
                cli.plain,
            )
            .await?;
        }
        None => {
            let urls = resolve_urls(cli.urls, cli.no_clipboard)?;
            if urls.is_empty() {
                if !cli.plain {
                    banner();
                }
                eprintln!("  No URL in args or clipboard.");
                eprintln!("  Copy a link, then:  junk");
                eprintln!("  Or:  junk <url>   ·   junk tui   ·   junk --audio <url>");
                eprintln!();
                // On macOS, nudge toward TUI
                if cfg!(target_os = "macos") {
                    eprintln!("  Mac tip:  junk tui  — arcade UI with auto-clipboard on [a]");
                }
                std::process::exit(2);
            }

            if !cli.plain {
                banner();
            }

            // Classify first URL for default dir
            let media = should_use_media(&urls[0], cli.stream || cli.audio, cli.http_only);
            let dir = if cli.ventoy {
                resolve_ventoy()?
            } else if let Some(d) = cli.dir {
                d
            } else if cli.audio {
                default_music_dir()
            } else if media {
                default_video_dir()
            } else {
                default_download_dir()
            };

            stage(&format!("dest {}", dir.display()));
            if find_yt_dlp().is_some() {
                stage("yt-dlp ready");
            } else {
                warn("yt-dlp not found — streaming sites need it (pacman/brew install yt-dlp)");
            }
            if find_ffmpeg().is_some() {
                stage("ffmpeg ready");
            } else {
                warn("ffmpeg not found — needed to merge video+audio");
            }
            eprintln!();

            run_jobs(
                urls,
                dir,
                connections,
                cli.quality,
                cli.audio,
                cli.http_only,
                cli.stream,
                cli.plain,
            )
            .await?;
        }
    }
    Ok(())
}

fn resolve_urls(mut urls: Vec<String>, no_clipboard: bool) -> Result<Vec<String>> {
    urls.retain(|u| !u.trim().is_empty());
    if urls.is_empty() && !no_clipboard {
        if let Some(u) = clipboard_url() {
            stage(&format!("clipboard → {u}"));
            urls.push(u);
        }
    }
    // normalize
    let urls: Vec<String> = urls
        .into_iter()
        .map(|u| {
            let n = normalize_paste(&u);
            if n.starts_with("www.") {
                format!("https://{n}")
            } else {
                n
            }
        })
        .filter(|u| looks_like_url(u))
        .collect();
    Ok(urls)
}

fn resolve_file_dir(dir: Option<PathBuf>, ventoy: bool) -> Result<PathBuf> {
    if let Some(d) = dir {
        return Ok(d);
    }
    if ventoy {
        return resolve_ventoy();
    }
    Ok(default_download_dir())
}

fn resolve_ventoy() -> Result<PathBuf> {
    let mounts = find_ventoy_mounts();
    if mounts.is_empty() {
        bail!("no Ventoy mount found under /run/media, /media, /mnt");
    }
    if mounts.len() > 1 {
        eprintln!("  found {} Ventoy mounts, using {}", mounts.len(), mounts[0].display());
    }
    Ok(mounts[0].clone())
}

#[allow(clippy::too_many_arguments)]
async fn run_jobs(
    urls: Vec<String>,
    dir: PathBuf,
    connections: u32,
    quality: u32,
    audio: bool,
    http_only: bool,
    force_stream: bool,
    plain: bool,
) -> Result<()> {
    std::fs::create_dir_all(&dir).with_context(|| format!("mkdir {}", dir.display()))?;

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let c = Arc::clone(&cancel);
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            c.store(true, Ordering::Relaxed);
            eprintln!("\n  cancel requested…");
        });
    }

    let mut any_fail = false;
    for (i, url) in urls.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            any_fail = true;
            break;
        }
        let job_id = (i + 1) as u64;
        eprintln!("  ── job {job_id}/{} ──", urls.len());
        stage(url);

        let use_media = should_use_media(url, force_stream || audio, http_only);
        let (tx, mut rx) = mpsc::channel::<ProgressEvent>(128);

        let cancel_j = Arc::clone(&cancel);
        let dir_c = dir.clone();
        let url_c = url.clone();

        let mut work = tokio::spawn(async move {
            if use_media {
                let mode = if audio {
                    MediaMode::Audio
                } else {
                    MediaMode::Video {
                        max_height: quality,
                    }
                };
                download_media(
                    &url_c,
                    mode,
                    &dir_c,
                    connections,
                    cancel_j,
                    tx,
                    job_id,
                )
                .await
            } else {
                let name = junk_core::sanitize_filename(
                    url_c
                        .split('/')
                        .next_back()
                        .unwrap_or("download.bin"),
                );
                // strip query
                let name = name.split('?').next().unwrap_or(&name).to_string();
                let dest = dir_c.join(if name.is_empty() {
                    "download.bin".into()
                } else {
                    name
                });
                let opts = DownloadOptions {
                    connections,
                    cancel: cancel_j,
                    pause: Arc::new(AtomicBool::new(false)),
                    job_id,
                };
                download_url(&url_c, &dest, opts, tx).await
            }
        });

        let mut last_pct_bucket = 0u32;
        loop {
            tokio::select! {
                ev = rx.recv() => {
                    if let Some(ev) = ev {
                        if !plain {
                            progress_line(&ev);
                            if ev.bytes_total > 0 {
                                let pct = (100.0 * ev.bytes_done as f64 / ev.bytes_total as f64) as u32;
                                let bucket = pct / 20;
                                if bucket > last_pct_bucket && bucket <= 5 {
                                    last_pct_bucket = bucket;
                                    eprintln!();
                                    print_syringe(pct as f32 / 100.0);
                                }
                            }
                        } else if matches!(ev.phase, Phase::Done | Phase::Error) {
                            eprintln!(
                                "[{:?}] {} {}/{}",
                                ev.phase,
                                ev.filename,
                                human_bytes(ev.bytes_done),
                                human_bytes(ev.bytes_total)
                            );
                        }
                    }
                }
                res = &mut work => {
                    while let Ok(ev) = rx.try_recv() {
                        if !plain {
                            progress_line(&ev);
                        }
                    }
                    match res {
                        Ok(Ok(path)) => {
                            if !plain {
                                print_syringe(1.0);
                            }
                            ok(&format!("{}", path.display()));
                        }
                        Ok(Err(e)) => {
                            eprintln!("  ✗ {e}");
                            any_fail = true;
                        }
                        Err(e) => {
                            eprintln!("  ✗ task: {e}");
                            any_fail = true;
                        }
                    }
                    break;
                }
            }
        }
        eprintln!();
    }

    if any_fail {
        bail!("one or more downloads failed");
    }
    Ok(())
}

