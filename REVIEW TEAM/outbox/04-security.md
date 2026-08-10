# SECURITY review
VERDICT: APPROVE

## Findings

### Secrets / signing (PASS — ship-critical)
- `.gitignore` correctly excludes `android/keystore/`, `android/keystore.properties`, `android/*.jks`, `android/*.keystore`.
- Local secrets exist only on disk (expected): `android/keystore/junk-fdroid.jks`, `SECRETS.txt`, `keystore.properties` — **not** present on GitHub `master` under `android/` (remote lists only `keystore.properties.example`).
- `app/build.gradle.kts` loads signing from gitignored properties or env (`JUNK_KEYSTORE*`); no hardcoded passwords in source.
- `keystore.properties.example` uses placeholders only.
- F-Droid metadata `scanignore` includes `android/keystore`.

### AndroidManifest / permissions
- `INTERNET` + `ACCESS_NETWORK_STATE`: required for a downloader.
- Legacy storage scoped with `maxSdkVersion` (WRITE ≤28, READ ≤32): correct.
- `MANAGE_EXTERNAL_STORAGE` (`minSdkVersion` 30): heavy-handed but **justified** for free write under public `Download/JUNK DRAWER`; **gated** in `MainActivity.ensureAllFilesAccessIfNeeded()` via `Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION` + on-screen “GRANT STORAGE IF NEEDED”. Matches CHANGESET risk note.
- `requestLegacyExternalStorage="true"`: fine for pre-R edge.
- `usesCleartextTraffic="true"` + `network_security_config` base cleartext: intentional for rare HTTP ISO/CDN links; HTTPS still preferred in practice. Acceptable for this product class; not a secret/exfil path by itself.
- No exported providers/receivers; single exported activity with LAUNCHER + SEND(text/plain) + VIEW(magnet|http|https). Share/VIEW only **paste** into the field (`handleIntent` → `pasteFromClipboard`); does not auto-start download. OK for a universal downloader.

### URL handling / injection (PASS)
- No `Runtime.exec` / `ProcessBuilder` / shell usage anywhere in Android or Rust sources.
- User URLs go only to OkHttp / `HttpURLConnection` / NewPipe Extractor — no command construction from URL strings.
- Scheme gate: magnets vs `http(s)` vs NewPipe host heuristics; bare strings get `https://` prefix. Not a shell injection surface.
- Magnet path: fixed `https://itorrents.org/torrent/$hash.torrent` then regex-extract webseeds; preferred names re-enter the HTTP downloader sanitizer.

### Filename / path traversal (PASS with residual)
- `MultiConnDownloader` sanitizes filenames: `[\\/:*?"<>|]` → `_`, max 180 chars, blank → `download.bin`.
- YouTube titles sanitized similarly before `preferredName`.
- Writes always under `JunkDrawer.dir()` = public Downloads/`JUNK DRAWER` (fixed folder name, not user-controlled).
- **Residual (non-blocking):** unlike Rust `junk-core` `filename_from_url` (rejects `.` / `..`), Android does **not** reject a basename of `.` or `..`. After separator strip, `File(drawer, "..")` still resolves to the parent of JUNK DRAWER. Practical impact is limited (directory open/delete usually fails; no multi-segment `../` after `/` strip), but should be hardened to match core.

### Other notes (non-blocking)
- `android:allowBackup="true"`: low risk for a downloader with no account secrets.
- `dist/*.apk` gitignored; consider also ignoring `dist/*.idsig` and other signing sidecars.
- Open VIEW filters for all `http`/`https` are expected for this app; no host allowlist needed.

## Stylistic decisions (if APPROVE)
- Keep cleartext permitted for direct file/CDN edge cases; do not force HTTPS-only if it breaks ISO mirrors.
- Keep `MANAGE_EXTERNAL_STORAGE` for visible public `JUNK DRAWER` rather than app-private storage that hides files from the user.
- Prefer honest magnet failures over silent “success.”

## Must-fix before re-review (if REJECT)
- (none — APPROVE)

## Recommended follow-ups (do not block this ship)
1. In `MultiConnDownloader` (and magnet `dn` before display/use), after sanitizer: if name is `.` or `..` or blank → `"download.bin"`; strip control/NUL chars like Rust `sanitize_filename`.
2. Optionally assert final `outFile.canonicalFile` is under `drawer.canonicalFile` before write.
3. Gitignore `dist/*.idsig` (hygiene).
