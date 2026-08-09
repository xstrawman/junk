//! junk — multi-connection HTTP(S) downloader + arcade TUI

mod arcade;
mod tui_app;

use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use junk_core::{
    default_download_dir, download_url, format_eta, human_bytes, human_rate, DownloadOptions,
    DownloadQueue, JobStatus, Phase, ProgressEvent,
};
use tokio::sync::mpsc;

#[derive(Parser, Debug)]
#[command(
    name = "junk",
    about = "Super-fast multi-connection HTTP(S) downloader — CLI + arcade syringe TUI",
    version
)]
struct Cli {
    /// Download directory (default: XDG_DOWNLOAD_DIR or ~/Downloads)
    #[arg(short = 'd', long = "dir", global = true)]
    dir: Option<PathBuf>,

    /// Parallel connections per file (1–32, default 16)
    #[arg(short = 'c', long = "connections", default_value_t = 16, global = true)]
    connections: u32,

    #[command(subcommand)]
    command: Option<Commands>,

    /// URLs to download (CLI mode when provided without subcommand)
    #[arg(trailing_var_arg = true, allow_hyphen_values = false)]
    urls: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Launch the retro arcade TUI
    Tui,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let dir = cli.dir.unwrap_or_else(default_download_dir);
    let connections = cli.connections.clamp(1, 32);

    match cli.command {
        Some(Commands::Tui) => {
            tui_app::run(dir, connections).await?;
        }
        None if cli.urls.is_empty() => {
            tui_app::run(dir, connections).await?;
        }
        None => {
            run_cli(dir, connections, cli.urls).await?;
        }
    }
    Ok(())
}

async fn run_cli(dir: PathBuf, connections: u32, urls: Vec<String>) -> Result<()> {
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("create download dir {}", dir.display()))?;

    let mut queue = DownloadQueue::new(dir.clone(), connections);
    for u in &urls {
        queue.enqueue(u).with_context(|| format!("enqueue {u}"))?;
    }

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let c = Arc::clone(&cancel);
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            c.store(true, Ordering::Relaxed);
        });
    }

    let tty = std::env::var_os("TERM").is_some() && std::env::var_os("CI").is_none();
    let mut any_fail = false;

    while let Some(job_id) = queue
        .jobs()
        .iter()
        .find(|j| j.status == JobStatus::Queued)
        .map(|j| j.id)
    {
        if cancel.load(Ordering::Relaxed) {
            any_fail = true;
            break;
        }

        let (url, dest) = {
            let j = queue
                .jobs_mut()
                .iter_mut()
                .find(|j| j.id == job_id)
                .expect("job");
            j.status = JobStatus::Running;
            (j.url.clone(), j.dest_path.clone())
        };

        let job_cancel = Arc::new(AtomicBool::new(false));
        let job_cancel_w = Arc::clone(&job_cancel);
        let global = Arc::clone(&cancel);
        let watch = tokio::spawn(async move {
            while !global.load(Ordering::Relaxed) && !job_cancel_w.load(Ordering::Relaxed) {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            if global.load(Ordering::Relaxed) {
                job_cancel_w.store(true, Ordering::Relaxed);
            }
        });

        let (tx, mut rx) = mpsc::channel::<ProgressEvent>(128);
        let opts = DownloadOptions {
            connections,
            cancel: Arc::clone(&job_cancel),
            pause: Arc::new(AtomicBool::new(false)),
            job_id,
        };

        let mut download = tokio::spawn(async move { download_url(&url, &dest, opts, tx).await });

        let mut last_line_len = 0usize;
        loop {
            tokio::select! {
                ev = rx.recv() => {
                    if let Some(ev) = ev {
                        if let Some(j) = queue.jobs_mut().iter_mut().find(|j| j.id == job_id) {
                            j.bytes_done = ev.bytes_done;
                            j.bytes_total = ev.bytes_total;
                            j.bytes_per_sec = ev.bytes_per_sec;
                            j.connections_active = ev.connections_active;
                        }
                        print_progress(&ev, tty, &mut last_line_len);
                    }
                }
                res = &mut download => {
                    while let Ok(ev) = rx.try_recv() {
                        print_progress(&ev, tty, &mut last_line_len);
                    }
                    if tty {
                        eprintln!();
                    }
                    job_cancel.store(true, Ordering::Relaxed);
                    let _ = watch.abort();
                    match res {
                        Ok(Ok(path)) => {
                            if let Some(j) = queue.jobs_mut().iter_mut().find(|j| j.id == job_id) {
                                j.status = JobStatus::Done;
                                j.dest_path = path;
                            }
                        }
                        Ok(Err(e)) => {
                            let msg = e.to_string();
                            let cancelled = matches!(e, junk_core::JunkError::Cancelled);
                            if let Some(j) = queue.jobs_mut().iter_mut().find(|j| j.id == job_id) {
                                j.status = if cancelled {
                                    JobStatus::Cancelled
                                } else {
                                    JobStatus::Failed
                                };
                                j.error = Some(msg.clone());
                            }
                            eprintln!("error: {msg}");
                            any_fail = true;
                            if cancelled {
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            any_fail = true;
                        }
                    }
                    break;
                }
            }
        }

        if cancel.load(Ordering::Relaxed) {
            break;
        }
    }

    for j in queue.jobs() {
        let st = match j.status {
            JobStatus::Done => "done",
            JobStatus::Failed => "FAIL",
            JobStatus::Cancelled => "cancelled",
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Paused => "paused",
        };
        eprintln!("  [{st}] {} → {}", j.url, j.dest_path.display());
        if let Some(err) = &j.error {
            eprintln!("         {err}");
        }
    }

    if any_fail {
        bail!("one or more downloads failed");
    }
    Ok(())
}

fn print_progress(ev: &ProgressEvent, tty: bool, last_len: &mut usize) {
    let pct = if ev.bytes_total > 0 {
        100.0 * ev.bytes_done as f64 / ev.bytes_total as f64
    } else {
        0.0
    };
    let line = format!(
        "[{:>5.1}%] {} / {}  {}  conn={}  eta={}  {}  ({:?})",
        pct,
        human_bytes(ev.bytes_done),
        if ev.bytes_total > 0 {
            human_bytes(ev.bytes_total)
        } else {
            "?".into()
        },
        human_rate(ev.bytes_per_sec),
        ev.connections_active,
        format_eta(ev.eta_secs),
        ev.filename,
        ev.phase,
    );

    if tty {
        let pad = if line.len() < *last_len {
            " ".repeat(*last_len - line.len())
        } else {
            String::new()
        };
        eprint!("\r{line}{pad}");
        let _ = io::stderr().flush();
        *last_len = line.len();
    } else if matches!(ev.phase, Phase::Done | Phase::Error | Phase::Finalizing) {
        eprintln!("{line}");
    }
}
