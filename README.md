# junk

**One product.** Hypersonic downloads:

- **aria2-style** multi-connection HTTP(S)
- **ytdl-style** streaming (yt-dlp resolve → multi-conn streams → **ffmpeg** merge)
- **Clipboard-first** CLI with cool ASCII art
- **Arcade TUI** still available (especially nice on **Mac** Terminal / iTerm)

```
  junk                         # grab URL from clipboard → go
  junk <url>                   # file or stream (auto-detect)
  junk --audio <url>           # MP3 → ~/Music  (ytdl-audio style)
  junk -q 1080 <youtube-url>   # max height 1080p
  junk --ventoy <iso-url>      # distrohopper ISO → Ventoy
  junk tui                     # full arcade TUI (Mac-friendly)
```

## How it decides

| Link type | What happens |
|-----------|----------------|
| YouTube / Vimeo / TikTok / … | yt-dlp gets stream URLs → **junk multi-conn** pulls them → ffmpeg merges (like your `ytdl`) |
| Direct ISO / zip / file | Pure multi-conn ranged GET |
| `--audio` | yt-dlp extract → mp3 in `~/Music` |
| `--http` | Force plain multi-conn (no yt-dlp) |
| `--stream` | Force media pipeline |

Needs **yt-dlp** + **ffmpeg** on `PATH` for streaming sites.

## CLI (default)

ASCII banner + syringe progress. No separate “product” — just run `junk`.

```bash
# copy a YouTube link, then:
junk

# or paste explicitly
junk 'https://www.youtube.com/watch?v=…'
junk 'https://example.com/big.iso'
```

## TUI (Mac & desktop terminals)

```bash
junk tui
```

- **`a`** — load clipboard into URL field  
- **Enter** — queue  
- Streams auto-route to `~/Videos`  
- Same engine as CLI  

## Android APK (arcade cabinet)

90s cabinet UI: paste **URL / MKV / magnet**, multi-conn download, syringe → cartoon brain-load line.

```bash
cd android
export ANDROID_HOME=~/Android/Sdk JAVA_HOME=/usr/lib/jvm/java-17-openjdk
./.gradle-dist/bin/gradle :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

Built APK (when present): `dist/junk-0.2.0-arcade-debug.apk`  
Details: `android/README.md`

**Note:** Characters are original yellow arcade noggins (not The Simpsons). Full DHT magnets need libtorrent in a follow-up; HTTP webseed magnets + direct files work.

## Install

### From source

```bash
cargo build --release -p junk
cp target/release/junk ~/bin/junk   # or ~/.local/bin
```

### Homebrew

```bash
brew tap xstrawman/junk https://github.com/xstrawman/junk
brew install junk
# also: brew install yt-dlp ffmpeg
```

### Flatpak

See `packaging/flatpak/`.

## Layout

| Crate / path | Role |
|--------------|------|
| `junk-core` | Multi-conn engine + media pipeline |
| `junk` | CLI (ASCII) + TUI front-ends |
| `Formula/junk.rb` | Homebrew |

## License

MIT
