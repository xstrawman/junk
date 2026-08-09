//! Arcade TUI front-end for junk-core.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
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
    let res = run_app(&mut terminal, dir, connections).await;
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
    // join handle for download task — we use channel only
    download_done: Option<tokio::sync::oneshot::Receiver<Result<PathBuf, String>>>,
    dir_prompt: bool,
}

impl App {
    fn new(dir: PathBuf, connections: u32) -> Self {
        Self {
            queue: DownloadQueue::new(dir.clone(), connections),
            input: String::new(),
            input_mode: false,
            status: format!("JUNK ARCADE — drop a URL  ·  dir {}", dir.display()),
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

        // mark running in queue
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
            let _ = done_tx.send(res.map_err(|e| e.to_string()).map(|p| p));
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

        let timeout = tick.saturating_sub(last.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
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
                        }
                        KeyCode::Enter => app.add_url(),
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
                            // second q quits
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
                    KeyCode::Char('a') => {
                        app.input_mode = true;
                        app.input.clear();
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
                        // Distrohopper: jump dest to Ventoy
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
        }
    }
    Ok(())
}

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();
    f.render_widget(
        Block::default().style(Style::default().bg(PAPER)),
        area,
    );

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

    // Title
    let title = Paragraph::new(Line::from(vec![
        Span::styled(" ⚡ JUNK ", title_style()),
        Span::styled(
            " multi-conn arcade  ",
            Style::default().fg(DIM),
        ),
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

    // Stage: syringe | arm
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

    // Scoreboard
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

    // Queue
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
            .title(format!(
                " QUEUE  (dir: {}) ",
                app.queue.dir().display()
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)),
    );
    f.render_widget(list, chunks[3]);

    // Input / status
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

    if let Some(err) = &app.error {
        // overlay hint
        let _ = err;
    }

    let help = Paragraph::new(Line::from(Span::styled(
        " a:add  p:pause  c:cancel  d:dir  v:ventoy  x:remove  j/k:select  q:quit ",
        Style::default().fg(DIM),
    )));
    f.render_widget(help, chunks[5]);
}
