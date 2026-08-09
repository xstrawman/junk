#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Connecting,
    Downloading,
    Finalizing,
    Done,
    Error,
}

#[derive(Debug, Clone)]
pub struct ProgressEvent {
    pub job_id: u64,
    pub filename: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub bytes_per_sec: f64,
    pub connections_active: u32,
    pub eta_secs: Option<u64>,
    pub phase: Phase,
    pub error: Option<String>,
}

impl ProgressEvent {
    pub fn eta(bytes_left: u64, rate: f64) -> Option<u64> {
        if rate <= 1.0 {
            return None;
        }
        Some((bytes_left as f64 / rate).ceil() as u64)
    }
}
