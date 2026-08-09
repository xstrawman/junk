# junk

Super-fast **multi-connection HTTP(S)** downloader (Rust, aria2-style ranges) with a
**retro arcade TUI** — a giant ASCII syringe loads a junkie’s arm as the download fills.

```
junk <url>              # CLI, multi-conn
junk                    # arcade TUI
junk tui                # same
junk -d ~/Downloads -c 16 <url>
```

## Build

```bash
cd ~/Projects/apps/junk
cargo build --release
cp target/release/junk ~/bin/junk
```

## CLI

```bash
junk https://example.com/big.iso
junk -c 32 https://a/file1 https://b/file2
```

- Parallel ranged GETs (default **16** connections, `--connections` / `-c`)
- Resume via `file.junk.part` + `file.junk.state.json`
- Saves to `$XDG_DOWNLOAD_DIR` or `~/Downloads` (`--dir` / `-d`)

## TUI keys

| Key | Action |
|-----|--------|
| `a` | Add URL |
| `p` | Pause / resume |
| `c` | Cancel active |
| `d` | Change download dir |
| `x` | Remove selected queued job |
| `j` / `k` | Move selection |
| `q` | Quit |

## Layout

- `crates/junk-core` — download engine
- `crates/junk` — CLI + ratatui arcade UI

## License

MIT
