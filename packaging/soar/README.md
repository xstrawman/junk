# Soar packaging for **junk**

[Soar](https://soar.qaidvoid.dev) is a distro-independent package manager for
static binaries and portable apps (no root). This tree is ready for
[soarpkgs](https://github.com/pkgforge/soarpkgs).

## Install Soar

```bash
curl -fsSL https://soar.qaidvoid.dev/install.sh | sh
export PATH="$HOME/.local/share/soar/bin:$PATH"
```

## Install junk

### A) Direct from GitHub Release (works today, once a release is published)

```bash
# x86_64
soar add \
  --name junk \
  --version 0.1.0 \
  --pkg-type static \
  https://github.com/xstrawman/junk/releases/download/v0.1.0/junk-x86_64-unknown-linux-musl

# aarch64
soar add \
  --name junk \
  --version 0.1.0 \
  --pkg-type static \
  https://github.com/xstrawman/junk/releases/download/v0.1.0/junk-aarch64-unknown-linux-musl
```

Soar tracks that release URL for updates.

### B) From soarpkgs (after PR merge)

```bash
soar sync
soar install junk
```

### C) Build from source (no soar)

```bash
cargo build --release
install -Dm755 target/release/junk ~/.local/bin/junk
```

## Package definition

| File | Role |
|------|------|
| `packages/junk/pkg.toml` | Identity, hosts, update strategy, artifact URL template |

Version pin files (`junk-<ver>.toml` with blake3 hashes) are produced by
[`sbuild resolve`](https://github.com/pkgforge/soarpkgs/blob/main/docs/FORMAT.md)
when submitting to soarpkgs — do not hand-edit hashes.

## Submit to official soarpkgs

```bash
git clone --filter=blob:none https://github.com/pkgforge/soarpkgs
cp -a packaging/soar/packages/junk soarpkgs/packages/junk
cd soarpkgs
# with sbuild installed:
sbuild resolve . junk
sbuild validate
# open a PR with packages/junk/
```

See: https://docs.pkgforge.dev/repositories/soarpkgs/contribution

## Release asset naming (required)

GitHub Releases must publish:

```
junk-x86_64-unknown-linux-musl
junk-aarch64-unknown-linux-musl
```

Produced by `.github/workflows/release.yml` on tags `v*`.
