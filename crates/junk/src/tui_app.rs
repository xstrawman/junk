//! Arcade TUI front-end for junk-core.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use crossterm::execute;
use junk_core::{
    distrohopper_line, download_url, find_ventoy_mounts, human_bytes, DownloadOptions,
    DownloadQueue, JobStatus, Phase, ProgressEvent,
};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::{DefaultTerminal, Frame};
use tokio::sync::mpsc;

use crate::arcade::{
    arm_lines, scoreboard_line, syringe_lines, title_style, DIM, NEON_AMBER, NEON_CYAN,
    NEON_GREEN, NEON_MAGENTA, NEON_RED, PAPER,
};

pub async fn run(dir: PathBuf, connections: u32) -> Result<()> {
    std::fs::create_dir_all(&dir)?;

    let mut terminal = ratatui::init();
    // Terminal paste (Shift+Ctrl+V / middle-click) arrives as Event::Paste
    let _ = execute!(io::stdout(), EnableBracketedPaste);
    let res = run_app(&mut terminal, dir, connections).await;
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    res
}

struct App {
    queue: DownloadQueue,
    input: String,
    input_mode: bool,
    status: String,
    error: Option<String>,
    success_flash: bool,
    selected: usize,
    anim_t: f32,
    progress: f32,
    last_ev: Option<ProgressEvent>,
    downloading: bool,
    cancel: Arc<AtomicBool>,
    pause: Arc<AtomicBool>,
    rx: Option<mpsc::Receiver<ProgressEvent>>,
    download_done: Option<tokio::sync::oneshot::Receiver<Result<PathBuf, String>>>,
    dir_prompt: bool,
}

impl App {
    fn new(dir: PathBuf, connections: u32) -> Self {
        Self {
            queue: DownloadQueue::new(dir.clone(), connections),
            input: String::new(),
            input_mode: false,
            status: format!(
                "JUNK ARCADE — press a (auto-pastes clipboard)  ·  dir {}",
                dir.display()
            ),
            error: None,
            success_flash: false,
            selected: 0,
            anim_t: 0.0,
            progress: 0.0,
            last_ev: None,
            downloading: false,
            cancel: Arc::new(AtomicBool::new(false)),
            pause: Arc::new(AtomicBool::new(false)),
            rx: None,
            download_done: None,
            dir_prompt: false,
        }
    }

    /// Open URL entry and deposit clipboard contents (first line, trimmed).
    fn start_add_from_clipboard(&mut self) {
        self.input_mode = true;
        self.dir_prompt = false;
        self.error = None;
        match clipboard_text() {
            Some(text) => {
                let line = normalize_paste(&text);
                if line.is_empty() {
                    self.input.clear();
                    self.status = "URL> (clipboard empty — type or paste)".into();
                } else {
                    self.input = line;
                    self.status = format!(
                        "clipboard loaded — Enter to queue  ({})",
                        truncate_display(&self.input, 48)
                    );
                }
            }
            None => {
                self.input.clear();
                self.status =
                    "URL> (no clipboard — type, or paste with Ctrl+Shift+V / Ctrl+V)".into();
            }
        }
    }

    fn apply_paste(&mut self, text: &str) {
        let line = normalize_paste(text);
        if line.is_empty() {
            return;
        }
        if self.dir_prompt {
            self.input.push_str(&line);
            return;
        }
        // Paste anywhere → URL field
        self.input_mode = true;
        self.input.push_str(&line);
        self.status = format!(
            "pasted — Enter to queue  ({})",
            truncate_display(&self.input, 48)
        );
    }

    fn paste_clipboard_into_input(&mut self) {
        if let Some(text) = clipboard_text() {
            let line = normalize_paste(&text);
            if !line.is_empty() {
                // Replace field when empty; append if user already typed
                if self.input.is_empty() {
                    self.input = line;
                } else {
                    self.input.push_str(&line);
                }
                self.status = format!(
                    "clipboard → field  ({})",
                    truncate_display(&self.input, 48)
                );
            }
        } else {
            self.error = Some("clipboard unavailable (try terminal paste)".into());
        }
    }

    fn add_url(&mut self) {
        let url = self.input.trim().to_string();
        if url.is_empty() {
            return;
        }
        match self.queue.enqueue(&url) {
            Ok(_) => {
                self.status = format!("queued {url}");
                self.error = None;
                self.input.clear();
                self.input_mode = false;
            }
            Err(e) => {
                self.error = Some(e.to_string());
            }
        }
    }

    fn start_download_if_idle(&mut self) {
        if self.downloading {
            return;
        }
        let job = self
            .queue
            .jobs()
            .iter()
            .find(|j| j.status == JobStatus::Queued)
            .cloned();
        let Some(job) = job else { return };

        if let Some(j) = self
            .queue
            .jobs_mut()
            .iter_mut()
            .find(|j| j.id == job.id)
        {
            j.status = JobStatus::Running;
        }

        self.cancel.store(false, Ordering::Relaxed);
        self.pause.store(false, Ordering::Relaxed);
        self.downloading = true;
        self.success_flash = false;
        self.error = None;
        self.progress = 0.0;
        self.status = format!("INJECTING {}", job.url);

        let (tx, rx) = mpsc::channel::<ProgressEvent>(256);
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        self.rx = Some(rx);
        self.download_done = Some(done_rx);

        let url = job.url.clone();
        let dest = job.dest_path.clone();
        let opts = DownloadOptions {
            connections: self.queue.connections(),
            cancel: Arc::clone(&self.cancel),
            pause: Arc::clone(&self.pause),
            job_id: job.id,
        };

        tokio::spawn(async move {
            let res = download_url(&url, &dest, opts, tx).await;
            let _ = done_tx.send(res.map_err(|e| e.to_string()));
        });
    }

    fn poll_download(&mut self) {
        if let Some(rx) = &mut self.rx {
            while let Ok(ev) = rx.try_recv() {
                if ev.bytes_total > 0 {
                    self.progress = (ev.bytes_done as f64 / ev.bytes_total as f64) as f32;
                }
                if let Some(j) = self
                    .queue
                    .jobs_mut()
                    .iter_mut()
                    .find(|j| j.id == ev.job_id)
                {
                    j.bytes_done = ev.bytes_done;
                    j.bytes_total = ev.bytes_total;
                    j.bytes_per_sec = ev.bytes_per_sec;
                    j.connections_active = ev.connections_active;
                }
                if matches!(ev.phase, Phase::Error) {
                    self.error = ev.error.clone();
                }
                self.last_ev = Some(ev);
            }
        }

        if let Some(done) = &mut self.download_done {
            match done.try_recv() {
                Ok(Ok(path)) => {
                    if let Some(j) = self
                        .queue
                        .jobs_mut()
                        .iter_mut()
                        .find(|j| j.status == JobStatus::Running)
                    {
                        j.status = JobStatus::Done;
                        j.dest_path = path.clone();
                    }
                    self.progress = 1.0;
                    self.downloading = false;
                    self.success_flash = true;
                    self.status = format!("LEVEL CLEAR — {}", path.display());
                    self.rx = None;
                    self.download_done = None;
                }
                Ok(Err(e)) => {
                    let cancelled = e.contains("cancelled");
                    if let Some(j) = self
                        .queue
                        .jobs_mut()
                        .iter_mut()
                        .find(|j| j.status == JobStatus::Running)
                    {
                        j.status = if cancelled {
                            JobStatus::Cancelled
                        } else {
                            JobStatus::Failed
                        };
                        j.error = Some(e.clone());
                    }
                    self.downloading = false;
                    if cancelled {
                        self.status = "cancelled — partial kept for resume".into();
                    } else {
                        self.error = Some(e.clone());
                        self.status = format!("TILT — {e}");
                    }
                    self.rx = None;
                    self.download_done = None;
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    self.downloading = false;
                    self.rx = None;
                    self.download_done = None;
                }
            }
        }
    }
}

/// First non-empty line, trimmed. Strips surrounding quotes often added by copy.
fn normalize_paste(text: &str) -> String {
    let line = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .trim_matches(|c| c == '"' || c == '\'' || c == '<' || c == '>')
        .trim()
        .to_string();
    line
}

fn truncate_display(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

/// System clipboard: arboard, then wl-paste / xclip / xsel fallbacks.
fn clipboard_text() -> Option<String> {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        if let Ok(t) = cb.get_text() {
            let t = t.trim().to_string();
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    for cmd in [
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

async fn run_app(terminal: &mut DefaultTerminal, dir: PathBuf, connections: u32) -> Result<()> {
    let mut app = App::new(dir, connections);
    let tick = Duration::from_millis(33);
    let mut last = Instant::now();

    loop {
        app.poll_download();
        app.start_download_if_idle();

        let dt = last.elapsed().as_secs_f32();
        last = Instant::now();
        app.anim_t += dt;

        terminal.draw(|f| ui(f, &app))?;
        let _ = io::stdout().flush();

        let timeout = tick.saturating_sub(last.elapsed());
        if !event::poll(timeout)? {
            continue;
        }

        match event::read()? {
            // Bracketed paste from terminal
            Event::Paste(text) => {
                app.apply_paste(&text);
            }
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // Ctrl+V / Ctrl+Shift+V → clipboard into field
                if matches!(key.code, KeyCode::Char('v') | KeyCode::Char('V'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    if !app.input_mode && !app.dir_prompt {
                        app.start_add_from_clipboard();
                    } else {
                        app.paste_clipboard_into_input();
                    }
                    continue;
                }

                if app.dir_prompt {
                    match key.code {
                        KeyCode::Esc => {
                            app.dir_prompt = false;
                            app.input.clear();
                        }
                        KeyCode::Enter => {
                            let p = PathBuf::from(app.input.trim());
                            if !app.input.trim().is_empty() {
                                let _ = std::fs::create_dir_all(&p);
                                app.queue.set_dir(p.clone());
                                app.status = format!("dir → {}", p.display());
                            }
                            app.dir_prompt = false;
                            app.input.clear();
                        }
                        KeyCode::Char(c) => app.input.push(c),
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        _ => {}
                    }
                    continue;
                }

                if app.input_mode {
                    match key.code {
                        KeyCode::Esc => {
                            app.input_mode = false;
                            app.input.clear();
                            app.status = "cancelled add".into();
                        }
                        KeyCode::Enter => app.add_url(),
                        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.input.clear();
                        }
                        KeyCode::Char(c) => app.input.push(c),
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        if app.downloading {
                            app.cancel.store(true, Ordering::Relaxed);
                            app.status = "cancelling… (q again to force quit)".into();
                            if event::poll(Duration::from_millis(400))? {
                                if let Event::Key(k2) = event::read()? {
                                    if matches!(k2.code, KeyCode::Char('q') | KeyCode::Esc) {
                                        break;
                                    }
                                }
                            }
                        } else {
                            break;
                        }
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        break;
                    }
                    KeyCode::Char('a') | KeyCode::Char('A') => {
                        // Auto-deposit clipboard into URL field
                        app.start_add_from_clipboard();
                    }
                    KeyCode::Char('p') => {
                        if app.pause.load(Ordering::Relaxed) {
                            app.pause.store(false, Ordering::Relaxed);
                            app.status = "resumed".into();
                        } else if app.downloading {
                            app.pause.store(true, Ordering::Relaxed);
                            app.status = "paused".into();
                        }
                    }
                    KeyCode::Char('c') => {
                        if app.downloading {
                            app.cancel.store(true, Ordering::Relaxed);
                            app.status = "cancel requested…".into();
                        }
                    }
                    KeyCode::Char('d') => {
                        app.dir_prompt = true;
                        app.input = app.queue.dir().display().to_string();
                    }
                    KeyCode::Char('v') => {
                        let mounts = find_ventoy_mounts();
                        if mounts.is_empty() {
                            app.error = Some(
                                "no Ventoy mount — plug in the stick of infinite reboots".into(),
                            );
                            app.status = distrohopper_line("no-ventoy-tui").into();
                        } else {
                            let dest = mounts[0].clone();
                            app.queue.set_dir(dest.clone());
                            app.error = None;
                            app.success_flash = true;
                            app.status = format!(
                                "VENTOY LOCKED → {}  ·  {}",
                                dest.display(),
                                distrohopper_line(&dest.display().to_string())
                            );
                        }
                    }
                    KeyCode::Char('x') => {
                        let jobs = app.queue.jobs();
                        if let Some(j) = jobs.get(app.selected) {
                            let id = j.id;
                            if app.queue.remove_queued(id) {
                                app.status = "removed from queue".into();
                                if app.selected > 0 {
                                    app.selected -= 1;
                                }
                            }
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        let n = app.queue.jobs().len();
                        if n > 0 {
                            app.selected = (app.selected + 1).min(n - 1);
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.selected = app.selected.saturating_sub(1);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();
    f.render_widget(Block::default().style(Style::default().bg(PAPER)), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(14),
            Constraint::Length(3),
            Constraint::Length(8),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    let title = Paragraph::new(Line::from(vec![
        Span::styled(" ⚡ JUNK ", title_style()),
        Span::styled(" multi-conn arcade  ", Style::default().fg(DIM)),
        Span::styled(
            "syringe → arm writes itself",
            Style::default().fg(NEON_CYAN),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(NEON_MAGENTA)),
    );
    f.render_widget(title, chunks[0]);

    let stage = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    let syn = syringe_lines(app.progress, app.anim_t);
    let syringe = Paragraph::new(syn).block(
        Block::default()
            .title(" SYRINGE ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(NEON_CYAN)),
    );
    f.render_widget(syringe, stage[0]);

    let arm = arm_lines(app.progress, app.anim_t);
    let arm_w = Paragraph::new(arm).block(
        Block::default()
            .title(" JUNKIE ARM ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(NEON_AMBER)),
    );
    f.render_widget(arm_w, stage[1]);

    let (rate, conn, eta, name) = if let Some(ev) = &app.last_ev {
        (
            ev.bytes_per_sec,
            ev.connections_active,
            ev.eta_secs,
            ev.filename.as_str(),
        )
    } else {
        (0.0, 0, None, "—")
    };
    let board = Paragraph::new(scoreboard_line(
        rate,
        conn,
        app.queue.connections(),
        eta,
        name,
    ))
    .block(
        Block::default()
            .title(" SCOREBOARD ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(NEON_GREEN)),
    );
    f.render_widget(board, chunks[2]);

    let items: Vec<ListItem> = app
        .queue
        .jobs()
        .iter()
        .enumerate()
        .map(|(i, j)| {
            let mark = match j.status {
                JobStatus::Running => "▶",
                JobStatus::Done => "✓",
                JobStatus::Failed => "✗",
                JobStatus::Cancelled => "⊘",
                JobStatus::Queued => "·",
                JobStatus::Paused => "❚❚",
            };
            let pct = if j.bytes_total > 0 {
                format!(
                    "{:>5.1}%",
                    100.0 * j.bytes_done as f64 / j.bytes_total as f64
                )
            } else {
                "  —  ".into()
            };
            let line = format!(
                " {mark} {pct}  {}  {}",
                j.url,
                if j.bytes_total > 0 {
                    format!(
                        "{} / {}",
                        human_bytes(j.bytes_done),
                        human_bytes(j.bytes_total)
                    )
                } else {
                    String::new()
                }
            );
            let style = if i == app.selected {
                Style::default()
                    .fg(NEON_AMBER)
                    .add_modifier(Modifier::BOLD)
            } else {
                match j.status {
                    JobStatus::Done => Style::default().fg(NEON_GREEN),
                    JobStatus::Failed => Style::default().fg(NEON_RED),
                    JobStatus::Running => Style::default().fg(NEON_MAGENTA),
                    _ => Style::default().fg(NEON_CYAN),
                }
            };
            ListItem::new(Line::from(Span::styled(line, style)))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(format!(" QUEUE  (dir: {}) ", app.queue.dir().display()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)),
    );
    f.render_widget(list, chunks[3]);

    let prompt = if app.dir_prompt {
        format!("DIR> {}_", app.input)
    } else if app.input_mode {
        format!("URL> {}_", app.input)
    } else {
        app.status.clone()
    };
    let color = if app.error.is_some() {
        NEON_RED
    } else if app.success_flash {
        NEON_GREEN
    } else {
        NEON_CYAN
    };
    let status = Paragraph::new(prompt)
        .style(Style::default().fg(color))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(color)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(status, chunks[4]);

    let help = Paragraph::new(Line::from(Span::styled(
        " a:add+clipboard  Enter:queue  Ctrl+V:paste  p:pause  c:cancel  d:dir  v:ventoy  q:quit ",
        Style::default().fg(DIM),
    )));
    f.render_widget(help, chunks[5]);
}
