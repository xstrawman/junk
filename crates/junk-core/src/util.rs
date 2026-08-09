use std::path::{Path, PathBuf};

pub fn default_download_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DOWNLOAD_DIR") {
        let p = PathBuf::from(xdg);
        if !p.as_os_str().is_empty() {
            return p;
        }
    }
    dirs::download_dir().unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Downloads")
    })
}

pub fn filename_from_url(url: &str) -> String {
    if let Ok(u) = url::Url::parse(url) {
        if let Some(seg) = u.path_segments() {
            if let Some(last) = seg.filter(|s| !s.is_empty()).last() {
                let decoded = percent_decode(last);
                let name = sanitize_filename(&decoded);
                if !name.is_empty() && name != "." && name != ".." {
                    return name;
                }
            }
        }
    }
    "download.bin".into()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

pub fn sanitize_filename(name: &str) -> String {
    let name = name.trim().trim_start_matches(['/', '\\']);
    let mut out: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    if out.len() > 200 {
        out.truncate(200);
    }
    if out.is_empty() {
        "download.bin".into()
    } else {
        out
    }
}

pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} {}", UNITS[i])
    } else {
        format!("{v:.2} {}", UNITS[i])
    }
}

pub fn human_rate(bps: f64) -> String {
    if bps < 0.0 || !bps.is_finite() {
        return "—".into();
    }
    format!("{}/s", human_bytes(bps as u64))
}

pub fn format_eta(secs: Option<u64>) -> String {
    match secs {
        None => "—".into(),
        Some(s) if s < 60 => format!("0:{s:02}"),
        Some(s) if s < 3600 => format!("{}:{:02}", s / 60, s % 60),
        Some(s) => format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60),
    }
}

/// Pick a unique final path if dest exists (unless we're resuming that exact part).
pub fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    for n in 1..1000 {
        let candidate = parent.join(format!("{stem}-{n}{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!("{stem}-dup{ext}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_path_chars() {
        assert_eq!(sanitize_filename("a/b\\c"), "a_b_c");
    }

    #[test]
    fn filename_from_simple_url() {
        assert_eq!(
            filename_from_url("https://example.com/files/big.iso"),
            "big.iso"
        );
    }

    #[test]
    fn human_bytes_units() {
        assert_eq!(human_bytes(512), "512 B");
        assert!(human_bytes(2048).contains("KiB"));
    }
}
