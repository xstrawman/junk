//! Retro arcade ASCII: giant syringe injects a junkie's arm as progress fills.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Neon arcade palette
pub const NEON_CYAN: Color = Color::Rgb(0, 255, 220);
pub const NEON_MAGENTA: Color = Color::Rgb(255, 0, 180);
pub const NEON_AMBER: Color = Color::Rgb(255, 200, 40);
pub const NEON_RED: Color = Color::Rgb(255, 60, 80);
pub const NEON_GREEN: Color = Color::Rgb(40, 255, 120);
pub const DIM: Color = Color::Rgb(80, 90, 110);
pub const PAPER: Color = Color::Rgb(12, 10, 22);

pub fn title_style() -> Style {
    Style::default()
        .fg(NEON_MAGENTA)
        .add_modifier(Modifier::BOLD)
}

pub fn label_style() -> Style {
    Style::default().fg(NEON_CYAN)
}

pub fn amber_style() -> Style {
    Style::default().fg(NEON_AMBER).add_modifier(Modifier::BOLD)
}

/// Build multi-line syringe art. `progress` 0.0–1.0 moves the plunger.
pub fn syringe_lines(progress: f32, anim_t: f32) -> Vec<Line<'static>> {
    let p = progress.clamp(0.0, 1.0);
    // plunger depth: more progress = more empty barrel (fluid gone into arm)
    let fluid_rows = 8;
    let remaining = ((1.0 - p) * fluid_rows as f32).ceil() as usize;
    let pulse = if (anim_t * 4.0).sin() > 0.0 { "═" } else { "─" };

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        "    ╔════╗",
        Style::default().fg(NEON_CYAN),
    )));
    lines.push(Line::from(Span::styled(
        format!("    ║ {pulse}{pulse} ║  PLUNGER"),
        Style::default().fg(DIM),
    )));
    lines.push(Line::from(Span::styled(
        "    ╠════╣",
        Style::default().fg(NEON_CYAN),
    )));

    for i in 0..fluid_rows {
        let from_bottom = fluid_rows - 1 - i;
        let filled = from_bottom < remaining;
        let body = if filled {
            Span::styled("████", Style::default().fg(NEON_MAGENTA))
        } else {
            Span::styled("····", Style::default().fg(DIM))
        };
        lines.push(Line::from(vec![
            Span::styled("    ║", Style::default().fg(NEON_CYAN)),
            body,
            Span::styled("║", Style::default().fg(NEON_CYAN)),
        ]));
    }

    lines.push(Line::from(Span::styled(
        "    ╚╗  ╔╝",
        Style::default().fg(NEON_CYAN),
    )));
    lines.push(Line::from(Span::styled(
        "     ║  ║   NEEDLE",
        Style::default().fg(DIM),
    )));
    lines.push(Line::from(Span::styled(
        "     ║  ║",
        Style::default().fg(NEON_CYAN),
    )));
    lines.push(Line::from(Span::styled(
        "     ╚══╝",
        Style::default().fg(NEON_AMBER),
    )));
    lines.push(Line::from(Span::styled(
        "      ||",
        Style::default().fg(NEON_AMBER),
    )));
    lines.push(Line::from(Span::styled(
        "      \\/",
        Style::default().fg(NEON_AMBER),
    )));
    lines
}

/// Arm silhouette that "writes itself" (fills) with progress.
pub fn arm_lines(progress: f32, anim_t: f32) -> Vec<Line<'static>> {
    let p = progress.clamp(0.0, 1.0);
    // arm template (right-pointing forearm + fist)
    let template: &[&str] = &[
        r"          .--.  ",
        r"      .--|    | ",
        r"  .--|  |    | ",
        r" |  |   |    | ",
        r" |  |   |    | ",
        r" |  |   |    | ",
        r"  '--|  |    | ",
        r"      '--|    | ",
        r"          '--'  ",
        r"    VEINS WRITING…",
    ];

    let total_chars: usize = template
        .iter()
        .map(|l| l.chars().filter(|c| !c.is_whitespace()).count())
        .sum();
    let reveal = ((p * total_chars as f32).ceil() as usize).min(total_chars);
    let mut seen = 0usize;
    let spark = if (anim_t * 6.0).sin() > 0.3 {
        "*"
    } else {
        "+"
    };

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        format!("  ARM LOAD  {spark}  {:>5.1}%", p * 100.0),
        amber_style(),
    )));

    for row in template {
        let mut spans = Vec::new();
        for ch in row.chars() {
            if ch.is_whitespace() {
                spans.push(Span::raw(ch.to_string()));
                continue;
            }
            if seen < reveal {
                let style = if p >= 1.0 {
                    Style::default().fg(NEON_GREEN).add_modifier(Modifier::BOLD)
                } else if p > 0.6 {
                    Style::default().fg(NEON_MAGENTA)
                } else {
                    Style::default().fg(NEON_CYAN)
                };
                spans.push(Span::styled(ch.to_string(), style));
                seen += 1;
            } else {
                spans.push(Span::styled(
                    if ch == '.' || ch == '\'' || ch == '-' || ch == '|' {
                        "·"
                    } else {
                        " "
                    }
                    .to_string(),
                    Style::default().fg(DIM),
                ));
                seen += 1;
            }
        }
        lines.push(Line::from(spans));
    }

    if p >= 1.0 {
        lines.push(Line::from(Span::styled(
            "  ★★ LEVEL CLEAR — HIT LOADED ★★",
            Style::default()
                .fg(NEON_GREEN)
                .add_modifier(Modifier::BOLD),
        )));
    } else if p <= 0.0 {
        lines.push(Line::from(Span::styled(
            "  waiting for juice…",
            Style::default().fg(DIM),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "  injecting…",
            Style::default().fg(NEON_MAGENTA),
        )));
    }

    lines
}

pub fn scoreboard_line(
    rate: f64,
    conn: u32,
    conn_max: u32,
    eta: Option<u64>,
    name: &str,
) -> Line<'static> {
    let eta_s = match eta {
        None => "—".to_string(),
        Some(s) if s < 60 => format!("0:{s:02}"),
        Some(s) if s < 3600 => format!("{}:{:02}", s / 60, s % 60),
        Some(s) => format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60),
    };
    let rate_s = junk_core::human_rate(rate);
    let name = name.to_string();
    Line::from(vec![
        Span::styled(" SPEED ", Style::default().fg(DIM)),
        Span::styled(rate_s, amber_style()),
        Span::styled("  CONN ", Style::default().fg(DIM)),
        Span::styled(
            format!("{conn}/{conn_max}"),
            Style::default().fg(NEON_CYAN),
        ),
        Span::styled("  ETA ", Style::default().fg(DIM)),
        Span::styled(eta_s, Style::default().fg(NEON_GREEN)),
        Span::styled("  ", Style::default()),
        Span::styled(name, Style::default().fg(NEON_MAGENTA)),
    ])
}
