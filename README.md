# junk

Super-fast **multi-connection HTTP(S)** downloader (Rust, aria2-style ranges) with a
**retro arcade TUI** — a giant ASCII syringe loads a junkie’s arm as the download fills.

```
junk <url>              # CLI, multi-conn
junk                    # arcade TUI
junk tui                # same
junk -d ~/Downloads -c 16 <url>

# Distrohopper express — mainline ISO straight to Ventoy
junk --ventoy https://…/ubuntu.iso
junk ventoy https://…/archlinux.iso
junk ventoy https://…/fedora.iso   # same idea, subcommand form
```

In the TUI, press **`v`** to lock the download dir onto a detected Ventoy mount.
(Identity is a temporary filesystem.)

## Install with [Soar](https://soar.qaidvoid.dev)

Soar is a rootless package manager for portable Linux binaries.

```bash
# install soar itself
curl -fsSL https://soar.qaidvoid.dev/install.sh | sh
export PATH="$HOME/.local/share/soar/bin:$PATH"

# install junk from a GitHub Release (x86_64 musl)
soar add --name junk --pkg-type static \
  https://github.com/xstrawman/junk/releases/download/v0.1.0/junk-x86_64-unknown-linux-musl
```

Package definition for [soarpkgs](https://github.com/pkgforge/soarpkgs):  
`packaging/soar/packages/junk/pkg.toml` — see `packaging/soar/README.md`.

## Install with Flatpak

```bash
flatpak install -y flathub org.freedesktop.Platform//24.08 org.freedesktop.Sdk//24.08
cd ~/Projects/apps/junk   # or clone the repo
flatpak-builder --user --install --force-clean \
  packaging/flatpak/build-dir \
  packaging/flatpak/dev.xstrawman.Junk.yml

flatpak run dev.xstrawman.Junk --ventoy https://example.com/distro.iso
```

Details: `packaging/flatpak/README.md`

## Build

```bash
cd ~/Projects/apps/junk
cargo build --release
cp target/release/junk ~/bin/junk

# portable musl binary (for Soar / releases)
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl -p junk
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
| `v` | **Ventoy** — dest = detected stick (distrohopper mode) |
| `x` | Remove selected queued job |
| `j` / `k` | Move selection |
| `q` | Quit |

## Layout

- `crates/junk-core` — download engine
- `crates/junk` — CLI + ratatui arcade UI

## License

MIT
