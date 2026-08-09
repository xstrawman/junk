//! Shared clipboard helpers (CLI + TUI). Linux: arboard / wl-paste / xclip.
//! macOS: arboard uses NSPasteboard.

use std::process::Command;

/// First non-empty line, trim quotes/angle brackets (common when copying links).
pub fn normalize_paste(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .trim_matches(|c| c == '"' || c == '\'' || c == '<' || c == '>')
        .trim()
        .to_string()
}

pub fn clipboard_text() -> Option<String> {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if let Ok(t) = cb.get_text() {
            let t = t.trim().to_string();
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    for cmd in [
        &["pbpaste"][..], // macOS
        &["wl-paste", "-n"][..],
        &["xclip", "-selection", "clipboard", "-o"][..],
        &["xsel", "--clipboard", "--output"][..],
    ] {
        if let Ok(out) = Command::new(cmd[0]).args(&cmd[1..]).output() {
            if out.status.success() {
                let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !t.is_empty() {
                    return Some(t);
                }
            }
        }
    }
    None
}

pub fn looks_like_url(s: &str) -> bool {
    let s = s.trim();
    s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("www.")
        || (s.contains('.') && !s.contains(' ') && s.len() > 4)
}

/// Clipboard URL if present and looks like a URL.
pub fn clipboard_url() -> Option<String> {
    let t = clipboard_text()?;
    let n = normalize_paste(&t);
    if looks_like_url(&n) {
        // promote www. to https
        if n.starts_with("www.") {
            Some(format!("https://{n}"))
        } else {
            Some(n)
        }
    } else {
        None
    }
}
