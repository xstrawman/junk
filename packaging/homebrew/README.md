# Homebrew packaging for **junk**

Formula lives at the repo root (Homebrew tap layout):

```
Formula/junk.rb
```

## Install

### Tap (recommended)

```bash
brew tap xstrawman/junk https://github.com/xstrawman/junk
brew install junk
```

Upgrade later:

```bash
brew update
brew upgrade junk
```

### HEAD (latest `master`)

```bash
brew tap xstrawman/junk https://github.com/xstrawman/junk
brew install --HEAD junk
```

### From a local clone

```bash
cd /path/to/junk
brew install --build-from-source ./Formula/junk.rb
```

### One-liner without a permanent tap

```bash
brew install --build-from-source \
  https://raw.githubusercontent.com/xstrawman/junk/master/Formula/junk.rb
```

## Requirements

- [Homebrew](https://brew.sh) (macOS or Linux)
- Formula builds with `cargo` (`depends_on "rust" => :build`)

No runtime Homebrew deps — pure Rust + rustls.

## Uninstall

```bash
brew uninstall junk
brew untap xstrawman/junk   # optional
```

## Maintainer notes

After tagging a release `vX.Y.Z`:

```bash
curl -fsSL -o /tmp/junk.tar.gz \
  "https://github.com/xstrawman/junk/archive/refs/tags/vX.Y.Z.tar.gz"
shasum -a 256 /tmp/junk.tar.gz
# put the hash in Formula/junk.rb `sha256`
# bump `version` to match
```

Livecheck (optional):

```bash
brew livecheck junk
```
