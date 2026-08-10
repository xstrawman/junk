# ARCHITECT review

VERDICT: APPROVE

## Scope checked

- `REVIEW TEAM/inbox/CHANGESET.md` goals vs tree
- `packaging/` (soar absence)
- Android module layout under `android/app/src/main/java/dev/xstrawman/junk/`
- Destination policy: `download/JunkDrawer.kt`, `MultiConnDownloader.kt`, HUD in `MainActivity.kt`
- YouTube/stream path: `download/YoutubeResolver.kt` + `JunkViewModel` routing
- Manifest / Gradle deps for NewPipe + storage permissions
- Root `README.md` + `android/README.md` product story
- Product invariants in `.grok/AGENTS.md`

## Product invariants

| Invariant | Result |
|-----------|--------|
| Public `Downloads/JUNK DRAWER` as primary APK destination | **PASS** — `JunkDrawer.FOLDER_NAME = "JUNK DRAWER"`, `RELATIVE_PATH = "Download/JUNK DRAWER"`, `dir()` uses `Environment.DIRECTORY_DOWNLOADS`; all multi-conn / magnet / extractable flows write via that dir |
| Scrapped Soar packaging gone | **PASS** — `packaging/` contains only `flatpak/` and `homebrew/`; no `packaging/soar`; product code has zero Soar references (mentions only in REVIEW TEAM docs / changeset) |
| YouTube / stream extract path exists on APK | **PASS** — NewPipe Extractor `0.24.6` in `app/build.gradle.kts`; `YoutubeResolver` + OkHttp downloader bridge; VM routes extractable hosts → resolve → multi-conn into JUNK DRAWER |

## Architecture findings

### Structure (good)

- **One product, multiple front-ends, shared concepts:** Desktop = `junk-core` + CLI/TUI; Android = Kotlin twin of the same ideas (multi-conn HTTP, clipboard/share inject, arcade framing). No pretence of a clever shared JNI layer — boring and shippable.
- **Clear module boundaries on APK:**
  - `download/` — engine only (`JunkDrawer`, `MultiConnDownloader`, `MagnetDownloader`, `YoutubeResolver`)
  - `ui/` — cabinet look (`ArcadeTheme`, `ArcadeStage`)
  - `JunkViewModel` — orchestration / URL triage / progress state
  - `MainActivity` — permissions, intents, clipboard, Compose shell
- **Destination policy is centralized:** Writers go through `JunkDrawer.dir()`; HUD always shows `savePath` / “JUNK DRAWER (all downloads)”. Path is not buried in app-private storage as the primary story.
- **Honest magnet architecture:** webseed best-effort with explicit failure text naming the missing DHT/libtorrent work — matches “prefer boring reliability / don’t fake success.”
- **Permissions aligned with destination:** legacy storage maxSdk gates + `MANAGE_EXTERNAL_STORAGE` for free write under public Download; UI exposes “GRANT STORAGE IF NEEDED.” Heavy-handed but coherent with the public-folder product choice.

### Tradeoffs accepted (not blockers)

- **Kotlin reimplementation of multi-conn** instead of binding `junk-core`: correct APK tradeoff for this stage; keep concepts aligned (chunked ranged GET, `.junk.part`, cancel, progress phases).
- **Adaptive YouTube without muxer:** progressive preferred; video-only fallback when no progressive — status string documents the limitation; matches known risk in changeset.
- **`JunkDrawer.insertPending` / `publish` unused:** MediaStore path currently leans on `MediaScannerConnection` after file write. Dead helpers are mild clutter, not an invariant break. Prefer one indexing story later (scan *or* pending insert), not both half-implemented.

### Coherence nits (non-blocking for this verdict)

1. **`android/README.md` is stale vs code** (product surface doc lag):
   - Still says files land in app-specific `Downloads/junk/` — **false**; code uses public `…/Download/JUNK DRAWER`.
   - Still says streaming/YouTube is “CLI/desktop via yt-dlp for now” — **false**; APK has NewPipe path.
   - Does not block architecture of the binary, but any human (or agent) reading only that README will violate product invariants in their head. **Should update before marketing / ship narrative**, ideally same cycle as QA.
2. **Root `README.md`** still centers desktop yt-dlp/ffmpeg and barely documents APK → JUNK DRAWER. Acceptable for a multi-front-end monorepo if `android/README.md` is fixed; root could gain one accurate APK bullet later.
3. **Desktop `default_download_dir()`** remains plain `~/Downloads` (no `JUNK DRAWER` subfolder). Changeset scope is APK destination policy; do not treat as APK reject. Longer-term product coherence may want the same drawer name on CLI/TUI if the brand is meant to be universal.

## Stylistic decisions (APPROVE)

- Keep public **`Download/JUNK DRAWER`** as the single APK destination; do not reintroduce app-private primary paths.
- Keep **NewPipe Extractor → multi-conn** as the phone stream path; do not reintroduce yt-dlp-on-device.
- Keep **Soar deleted**; packaging surface stays Flatpak + Homebrew (+ APK/F-Droid).
- Keep arcade HUD always showing full drawer path; neon copy (TILT / LEVEL CLEAR / INJECTING) is product language, not accidental.
- Prefer file-then-scan over half-wired MediaStore pending rows until one path is finished.
- Magnet = honest webseed-only until libtorrent lands; no fake “100% magnet” claims.

## Must-fix before re-review

_(none for architecture invariants)_

Recommended follow-ups (do not require architect re-REJECT):

- Fix `android/README.md` lines about save path and YouTube so docs match code invariants.
- Delete or wire `insertPending`/`publish` so MediaStore story is singular.

## Summary

Code architecture matches the product story this changeset claims: Soar is gone, downloads are forced into a visible public **JUNK DRAWER**, and a real YouTube/stream extract path exists and feeds multi-conn. Module boundaries are small and clear. Ship architecture is sound; refresh the stale Android README so documentation does not contradict the invariants.
