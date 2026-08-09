//! Multi-connection HTTP(S) download engine (aria2-style ranged GETs).

mod download;
mod progress;
mod resume;
mod util;

pub use download::{download_url, DownloadOptions};
pub use progress::{Phase, ProgressEvent};
pub use util::{
    default_download_dir, distrohopper_line, find_ventoy_mounts, format_eta, human_bytes,
    human_rate, sanitize_filename,
};

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;

#[derive(Debug, Error)]
pub enum JunkError {
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, JunkError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    Running,
    Paused,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: u64,
    pub url: String,
    pub dest_path: PathBuf,
    pub status: JobStatus,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub bytes_per_sec: f64,
    pub connections_active: u32,
    pub error: Option<String>,
}

impl Job {
    pub fn new(id: u64, url: String, dest_path: PathBuf) -> Self {
        Self {
            id,
            url,
            dest_path,
            status: JobStatus::Queued,
            bytes_done: 0,
            bytes_total: 0,
            bytes_per_sec: 0.0,
            connections_active: 0,
            error: None,
        }
    }
}

/// FIFO queue with one active download.
pub struct DownloadQueue {
    next_id: u64,
    jobs: Vec<Job>,
    dir: PathBuf,
    connections: u32,
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
}

impl DownloadQueue {
    pub fn new(dir: PathBuf, connections: u32) -> Self {
        Self {
            next_id: 1,
            jobs: Vec::new(),
            dir,
            connections: connections.clamp(1, 32),
            cancel: Arc::new(AtomicBool::new(false)),
            pause: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn dir(&self) -> &PathBuf {
        &self.dir
    }

    pub fn set_dir(&mut self, dir: PathBuf) {
        self.dir = dir;
    }

    pub fn connections(&self) -> u32 {
        self.connections
    }

    pub fn set_connections(&mut self, n: u32) {
        self.connections = n.clamp(1, 32);
    }

    pub fn jobs(&self) -> &[Job] {
        &self.jobs
    }

    pub fn jobs_mut(&mut self) -> &mut [Job] {
        &mut self.jobs
    }

    pub fn active_job(&self) -> Option<&Job> {
        self.jobs.iter().find(|j| j.status == JobStatus::Running)
    }

    pub fn enqueue(&mut self, url: &str) -> Result<u64> {
        let url = url.trim();
        if url.is_empty() {
            return Err(JunkError::InvalidUrl("empty".into()));
        }
        let parsed = url::Url::parse(url)
            .map_err(|e| JunkError::InvalidUrl(e.to_string()))?;
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            return Err(JunkError::InvalidUrl(format!(
                "only http/https supported, got {}",
                parsed.scheme()
            )));
        }
        let name = util::filename_from_url(url);
        let dest = self.dir.join(&name);
        let id = self.next_id;
        self.next_id += 1;
        self.jobs.push(Job::new(id, url.to_string(), dest));
        Ok(id)
    }

    pub fn remove_queued(&mut self, id: u64) -> bool {
        if let Some(pos) = self.jobs.iter().position(|j| j.id == id) {
            if self.jobs[pos].status == JobStatus::Queued {
                self.jobs.remove(pos);
                return true;
            }
        }
        false
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn request_pause(&self) {
        self.pause.store(true, Ordering::Relaxed);
    }

    pub fn request_resume(&self) {
        self.pause.store(false, Ordering::Relaxed);
    }

    pub fn is_paused(&self) -> bool {
        self.pause.load(Ordering::Relaxed)
    }

    pub fn clear_cancel(&self) {
        self.cancel.store(false, Ordering::Relaxed);
    }

    /// Run next queued job to completion (or fail/cancel). Returns true if a job ran.
    pub async fn run_next(
        &mut self,
        progress: mpsc::Sender<ProgressEvent>,
    ) -> Result<bool> {
        let idx = match self
            .jobs
            .iter()
            .position(|j| j.status == JobStatus::Queued)
        {
            Some(i) => i,
            None => return Ok(false),
        };

        self.clear_cancel();
        self.pause.store(false, Ordering::Relaxed);
        self.jobs[idx].status = JobStatus::Running;
        self.jobs[idx].error = None;

        let url = self.jobs[idx].url.clone();
        let dest = self.jobs[idx].dest_path.clone();
        let job_id = self.jobs[idx].id;
        let opts = DownloadOptions {
            connections: self.connections,
            cancel: Arc::clone(&self.cancel),
            pause: Arc::clone(&self.pause),
            job_id,
        };

        let result = download_url(&url, &dest, opts, progress).await;

        match result {
            Ok(final_path) => {
                self.jobs[idx].dest_path = final_path;
                self.jobs[idx].status = JobStatus::Done;
                self.jobs[idx].bytes_done = self.jobs[idx].bytes_total.max(1);
                Ok(true)
            }
            Err(JunkError::Cancelled) => {
                self.jobs[idx].status = JobStatus::Cancelled;
                Ok(true)
            }
            Err(e) => {
                self.jobs[idx].status = JobStatus::Failed;
                self.jobs[idx].error = Some(e.to_string());
                Err(e)
            }
        }
    }

}
