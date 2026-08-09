use std::fs;
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

/// Detect mounted Ventoy volumes (same heuristic as junkydoc-sync).
/// Looks under `/run/media`, `/media`, `/mnt` for labels/markers containing "ventoy".
pub fn find_ventoy_mounts() -> Vec<PathBuf> {
    let mut mounts = Vec::new();
    let candidates = [
        PathBuf::from("/run/media"),
        PathBuf::from("/media"),
        PathBuf::from("/mnt"),
    ];

    for root in candidates {
        if !root.is_dir() {
            continue;
        }
        if let Ok(users) = fs::read_dir(&root) {
            for user in users.flatten() {
                let user_path = user.path();
                if !user_path.is_dir() {
                    continue;
                }
                if let Ok(entries) = fs::read_dir(&user_path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() && looks_like_ventoy(&p) {
                            mounts.push(p);
                        }
                    }
                }
                if looks_like_ventoy(&user_path) {
                    mounts.push(user_path);
                }
            }
        }
    }

    mounts.sort();
    mounts.dedup();
    mounts
}

fn looks_like_ventoy(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.contains("ventoy") {
        return true;
    }
    path.join("ventoy").is_dir()
        || path.join("Ventoy").is_dir()
        || path.join("ventoy.json").is_file()
}

/// Distrohopper wisdom. Rotates on a cheap hash of the path/url so it feels intentional.
pub fn distrohopper_line(context: &str) -> &'static str {
    const LINES: &[&str] = &[
        "distrohopper mode: mainline ISO → Ventoy. identity is a temporary filesystem.",
        "another ISO for the stick of infinite reboots. the hopper never settles.",
        "downloading directly to Ventoy — skipping the 'save to Downloads then forget' epoch.",
        "tip: your current distro has 48 hours to impress you.",
        "Ventoy doesn't judge. it just accumulates your personality disorders as .iso files.",
        "mainline only? bold of you to pretend you won't also grab a GNOME rebuild at 3am.",
        "one stick to hop them all. eject when the rice gets cold.",
        "ISO landing on Ventoy. tomorrow's you can argue with today's you in the boot menu.",
        "multi-conn into Ventoy: the only commitment issues that improve your uptime.",
        "welcome back. the stick remembers every distro you swore was 'the one'.",
    ];
    let mut h: u32 = 2166136261;
    for b in context.as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(16777619);
    }
    LINES[(h as usize) % LINES.len()]
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
