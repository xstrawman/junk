# Flatpak packaging for **junk**

App ID: `dev.xstrawman.Junk`

## Permissions (why these exist)

| Finish arg | Reason |
|------------|--------|
| `--share=network` | HTTP(S) downloads |
| `xdg-download` | Default save dir |
| `/run/media`, `/media`, `/mnt` | Ventoy / USB mounts (distrohopper) |
| `home` | `junk -d ~/…` |

## Quick install (prebuilt musl binary)

Needs: `flatpak`, `flatpak-builder`, and Freedesktop 24.08 runtime/SDK.

```bash
# once
flatpak install -y flathub org.freedesktop.Platform//24.08 org.freedesktop.Sdk//24.08

# from repo root
cd ~/Projects/apps/junk
flatpak-builder --user --install --force-clean \
  packaging/flatpak/build-dir \
  packaging/flatpak/dev.xstrawman.Junk.yml
```

## Run

```bash
flatpak run dev.xstrawman.Junk
flatpak run dev.xstrawman.Junk https://proof.ovh.net/files/1Mb.dat
flatpak run dev.xstrawman.Junk --ventoy https://example.com/ubuntu.iso

# optional alias
alias junk='flatpak run dev.xstrawman.Junk'
```

## From source (dev)

```bash
flatpak install -y flathub org.freedesktop.Sdk.Extension.rust-stable//24.08

flatpak-builder --user --install --force-clean \
  packaging/flatpak/build-src \
  packaging/flatpak/dev.xstrawman.Junk.from-source.yml
```

Uses network during `cargo build` — fine for local use, **not** for Flathub as-is.

## Files

| File | Role |
|------|------|
| `dev.xstrawman.Junk.yml` | Binary Flatpak (Release asset) |
| `dev.xstrawman.Junk.from-source.yml` | Cargo build from checkout |
| `dev.xstrawman.Junk.desktop` | Desktop entry (Terminal=true) |
| `dev.xstrawman.Junk.metainfo.xml` | AppStream metadata |

## Flathub later

1. Generate offline Cargo sources with [flatpak-builder-tools](https://github.com/flatpak/flatpak-builder-tools) `cargo` generator  
2. Replace the binary module with a full source build  
3. Submit under a verified domain app-id if required  

## Uninstall

```bash
flatpak uninstall --user dev.xstrawman.Junk
```
