# Junk — multi-connection HTTP downloader + arcade TUI

**Date:** 2026-08-09  
**Status:** Approved for planning  
**Related:** sibling spirit of `junkydoc-sync` (local ISO→Ventoy), but **no** R. Crumb theme; this is a network downloader with a retro arcade TUI.

## Goal

Ship a Rust **aria2c-style** HTTP(S) multi-connection downloader named **junk**, with:

1. **CLI:** `junk <url>` — downloads super fast (parallel range requests).
2. **TUI:** retro arcade UI where a **giant syringe** injects a **junkie’s arm**; progress “writes itself” as the arm fills / injection completes.
3. **Queue:** one active download; waiting jobs listed below.

Not in v1: torrents/magnets, egui GUI, R. Crumb clinic art, wrapping system `aria2c`.

## Architecture

Workspace at `~/Projects/apps/junk/`:

```
junk/
  Cargo.toml                 # workspace
  crates/
    junk-core/               # download library
    junk/                    # binary: CLI + TUI
  docs/superpowers/specs/    # this design
```

| Invocation | Behavior |
|---|---|
| `junk <url> [url…]` | CLI mode: enqueue URLs, print progress, exit when queue empty |
| `junk` / `junk tui` | Arcade TUI: add URLs, syringe animation, manage queue |
| `junk --dir <path> …` | Override download directory |
| `junk --connections N` | Segment concurrency (default 16, clamp 1–32) |

**Data flow**

```
CLI or TUI
    → enqueue Job
    → junk-core worker (multi-range GET, assemble)
    → ProgressEvent channel
    → CLI lines  OR  TUI syringe/arm + stats
```

**Defaults**

- Destination: `XDG_DOWNLOAD_DIR` if set, else `~/Downloads`
- Resume via `.junk.part` + sidecar state file
- One worker processes the active job; queue is FIFO

## Engine (`junk-core`)

### Job model

```text
Job {
  id, url, dest_path,
  status: Queued | Running | Paused | Done | Failed | Cancelled,
  bytes_done, bytes_total, bytes_per_sec,
  connections_active, error: Option<String>
}
```

### Multi-connection algorithm

1. Probe with `HEAD` (or ranged `GET` if HEAD weak) → content length + `Accept-Ranges`.
2. If size known **and** ranges supported → split into **N** segments (default 16).
3. Concurrent ranged `GET`s write at correct file offsets into a preallocated partial file.
4. If ranges unsupported or size unknown → single-stream fallback.
5. Segment errors: retry with short backoff (few attempts); persistent failure fails the job with a clear message.
6. Follow redirects with a sane max hop count; HTTPS via rustls.

### Resume

- Temp file: `filename.junk.part` (or `basename` from URL/`Content-Disposition`).
- Sidecar: `filename.junk.state.json` — url, total size, etag/last-modified if present, segment map (start, end, done).
- On restart: skip completed ranges; re-fetch holes only.
- Success: fsync best-effort → rename `.part` → final name → remove sidecar.

### Progress events

Emitted frequently enough for smooth TUI (~10–20 Hz max, rate-limited):

- `bytes_done`, `bytes_total`, `bytes_per_sec`, `connections_active`
- `eta_secs`, `filename`, `job_id`
- `phase`: Connecting | Downloading | Finalizing | Done | Error

### Libraries (v1)

- `tokio` — async runtime, concurrent segments
- `reqwest` (rustls-tls) — HTTP client
- `serde` / `serde_json` — resume sidecar
- No dependency on system `aria2c`

### CLI output

- Human: one updating line or periodic lines: percent, rate, ETA, connections
- Non-TTY: plain progress lines without carriage-return games
- Exit code: `0` all success; non-zero if any job failed/cancelled

## Arcade TUI

### Stack

- `ratatui` + `crossterm` for the terminal UI
- Same `junk-core` worker as CLI (shared library)
- Tick/redraw on progress events + animation timer (~15–30 FPS when active)

### Visual metaphor

Retro **arcade cabinet** palette (neon cyan, magenta, amber on near-black):

- **Giant syringe** (ASCII/block art) on the left or top — plunger moves with progress
- **Junkie’s arm** — veins / fill bar “writes itself” as `bytes_done / bytes_total` increases (character-by-character or block fill of the arm silhouette)
- **Scoreboard** strip: rate (MB/s), connections, ETA, filename — arcade fonts via bold/colored text, not real bitmap fonts
- **Queue panel** under the stage: waiting jobs; active job highlighted
- When complete: flash “LEVEL CLEAR” / full arm + empty syringe; on error: “TILT” / red fault line

ASCII art is **data-driven** from progress (0.0–1.0), not a static splash. Multiple frames or fill levels (e.g. 0–10 stages) so motion reads clearly at a glance.

### Layout (80×24 minimum; richer at larger sizes)

```
┌─ JUNK ── multi-conn arcade ─────────────────────────────┐
│  [SYRINGE ASCII]          [ARM FILL ASCII]              │
│       ||                         ████░░░░               │
│      [==]  67%                   veins writing…         │
│                                                         │
│  SPEED 42.1 MB/s   CONN 12/16   ETA 0:41   file.iso     │
├─ QUEUE ─────────────────────────────────────────────────┤
│  ▶ running  bigfile.iso     67%  42 MB/s                │
│    queued   other.zip                                   │
│    done     small.bin                                   │
├─ INPUT ─────────────────────────────────────────────────┤
│  URL> _                                                 │
└─ keys: a add  p pause  c cancel  d dir  q quit ─────────┘
```

### Keybindings

| Key | Action |
|-----|--------|
| `a` / paste Enter in URL field | Add URL to queue |
| `p` | Pause / resume active job |
| `c` | Cancel active job |
| `j` / `k` or arrows | Move queue selection |
| `x` | Remove selected queued job |
| `d` | Change download directory (prompt) |
| `q` / `Esc` | Quit (confirm if download active) |

Clipboard paste of a URL into the input line is supported when the terminal provides it.

## Error handling

| Situation | Behavior |
|-----------|----------|
| Network blip on one segment | Retry segment; keep others running |
| 404 / 403 / permanent HTTP error | Fail job; show message; start next queued |
| Disk full | Fail job with clear error; leave `.part` for resume after free space |
| Invalid URL | Reject at enqueue; do not start worker |
| Ctrl+C / quit mid-download | Cooperative cancel; partial + sidecar kept for resume |
| No range support | Silent single-connection fallback; still show progress |

## Testing

**junk-core (priority)**

- Unit: segment split math, path sanitization from URL, human rate helpers
- Integration with a local test HTTP server (range + non-range):
  - multi-conn full download matches known bytes (checksum)
  - resume after simulated interrupt
  - cancel mid-download leaves consistent sidecar
  - redirect follow
- Single-stream fallback when server ignores ranges

**CLI**

- `junk` with file:// or localhost URL in tests; exit codes

**TUI**

- Light smoke: app boots, renders frames with mock progress (optional; not blocking v1 if flaky in CI)
- Manual check: syringe/arm fill tracks 0→100%

## Non-goals (v1)

- BitTorrent / magnet
- Metalink
- Browser extension / system tray
- egui or other native GUI
- R. Crumb / clinic patient metaphor
- Throttling schedules, per-site cookies UI (may add headers later if needed)
- Checksum verification from external .sha256 files (optional later)

## Success criteria

1. `junk https://…/large.iso` finishes correctly and is clearly multi-connection when the server allows ranges.
2. Interrupted download resumes without re-fetching completed ranges.
3. TUI shows colorful syringe→arm progress that tracks real bytes; queue runs one-at-a-time.
4. No dependency on `aria2c`; pure Rust stack.
5. Builds with `cargo build --release` on Linux; binary installable to `~/bin/junk`.

## Implementation order (for planning)

1. Workspace + `junk-core` API + multi-conn download + resume
2. CLI front-end
3. TUI shell + queue UX
4. ASCII syringe/arm animation bound to progress
5. Tests + README + optional desktop/term launcher note
