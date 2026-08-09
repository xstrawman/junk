//! Terminal ASCII art progress — one product, looks cool in any terminal (incl. Mac).

use std::io::{self, Write};

use junk_core::{format_eta, human_bytes, human_rate, Phase, ProgressEvent};

pub fn banner() {
    eprintln!(
        r#"
 ╔══════════════════════════════════════════════════════════════╗
 ║   ██╗██╗   ██╗███╗   ██╗██╗  ██╗                             ║
 ║   ██║██║   ██║████╗  ██║██║ ██╔╝   multi-conn · streams      ║
 ║   ██║██║   ██║██╔██╗ ██║█████╔╝    aria2 × ytdl × ffmpeg     ║
 ║██╗██║██║   ██║██║╚██╗██║██╔═██╗                              ║
 ║╚████╔╝╚██████╔╝██║ ╚████║██║  ██╗  paste a link. go hypersonic║
 ║ ╚═══╝  ╚═════╝ ╚═╝  ╚═══╝╚═╝  ╚═╝                            ║
 ╚══════════════════════════════════════════════════════════════╝"#
    );
}

pub fn print_syringe(progress: f32) {
    let p = progress.clamp(0.0, 1.0);
    let rows = 6usize;
    let filled = ((1.0 - p) * rows as f32).ceil() as usize; // fluid left in barrel
    eprintln!("   ╔════╗");
    eprintln!("   ║ == ║  plunger");
    eprintln!("   ╠════╣");
    for i in 0..rows {
        let from_bottom = rows - 1 - i;
        let cell = if from_bottom < filled {
            "████"
        } else {
            "····"
        };
        eprintln!("   ║{cell}║");
    }
    eprintln!("   ╚╗  ╔╝");
    eprintln!("    ║  ║   → arm load {:>5.1}%", p * 100.0);
    eprintln!("    ╚══╝");
}

/// Single updating status line for downloads.
pub fn progress_line(ev: &ProgressEvent) {
    let pct = if ev.bytes_total > 0 {
        100.0 * ev.bytes_done as f64 / ev.bytes_total as f64
    } else {
        0.0
    };
    let bar_w = 24usize;
    let filled = if ev.bytes_total > 0 {
        ((pct / 100.0) * bar_w as f64).round() as usize
    } else {
        0
    };
    let bar: String = (0..bar_w)
        .map(|i| if i < filled { '█' } else { '░' })
        .collect();

    let line = format!(
        "\r  [{bar}] {pct:>5.1}%  {} / {}  {}  conn={}  eta={}  {} ({:?})    ",
        human_bytes(ev.bytes_done),
        if ev.bytes_total > 0 {
            human_bytes(ev.bytes_total)
        } else {
            "?".into()
        },
        human_rate(ev.bytes_per_sec),
        ev.connections_active,
        format_eta(ev.eta_secs),
        truncate(&ev.filename, 40),
        ev.phase,
    );
    eprint!("{line}");
    let _ = io::stderr().flush();

    if matches!(ev.phase, Phase::Done | Phase::Error | Phase::Finalizing) {
        eprintln!();
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

pub fn stage(msg: &str) {
    eprintln!("  → {msg}");
}

pub fn ok(msg: &str) {
    eprintln!("  ✓ {msg}");
}

pub fn warn(msg: &str) {
    eprintln!("  ! {msg}");
}
