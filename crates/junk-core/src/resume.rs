use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{JunkError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentState {
    pub start: u64,
    pub end: u64, // inclusive
    pub done: u64, // bytes written for this segment
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeState {
    pub url: String,
    pub total_size: u64,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub segments: Vec<SegmentState>,
}

impl ResumeState {
    pub fn bytes_done(&self) -> u64 {
        self.segments.iter().map(|s| s.done.min(s.len())).sum()
    }
}

impl SegmentState {
    pub fn len(&self) -> u64 {
        self.end.saturating_sub(self.start).saturating_add(1)
    }

    pub fn remaining_start(&self) -> u64 {
        self.start + self.done
    }

    pub fn is_complete(&self) -> bool {
        self.done >= self.len()
    }
}

pub fn part_path(final_path: &Path) -> PathBuf {
    let mut s = final_path.as_os_str().to_os_string();
    s.push(".junk.part");
    PathBuf::from(s)
}

pub fn state_path(final_path: &Path) -> PathBuf {
    let mut s = final_path.as_os_str().to_os_string();
    s.push(".junk.state.json");
    PathBuf::from(s)
}

pub fn load_state(path: &Path) -> Result<Option<ResumeState>> {
    let p = state_path(path);
    if !p.exists() {
        return Ok(None);
    }
    let data = std::fs::read_to_string(&p)?;
    let st: ResumeState = serde_json::from_str(&data)
        .map_err(|e| JunkError::Other(format!("bad resume state: {e}")))?;
    Ok(Some(st))
}

pub fn save_state(path: &Path, state: &ResumeState) -> Result<()> {
    let p = state_path(path);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(state)
        .map_err(|e| JunkError::Other(e.to_string()))?;
    std::fs::write(p, data)?;
    Ok(())
}

pub fn clear_state(path: &Path) {
    let _ = std::fs::remove_file(state_path(path));
}

pub fn split_segments(total: u64, n: u32) -> Vec<SegmentState> {
    if total == 0 {
        return vec![];
    }
    let n = (n as u64).clamp(1, total).min(32);
    let chunk = total / n;
    let mut segs = Vec::with_capacity(n as usize);
    for i in 0..n {
        let start = i * chunk;
        let end = if i == n - 1 {
            total - 1
        } else {
            (i + 1) * chunk - 1
        };
        segs.push(SegmentState {
            start,
            end,
            done: 0,
        });
    }
    segs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_covers_all_bytes() {
        let segs = split_segments(1000, 4);
        assert_eq!(segs.len(), 4);
        assert_eq!(segs[0].start, 0);
        assert_eq!(segs[3].end, 999);
        let covered: u64 = segs.iter().map(|s| s.len()).sum();
        assert_eq!(covered, 1000);
    }
}
