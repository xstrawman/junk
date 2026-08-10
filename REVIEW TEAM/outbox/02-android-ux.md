# ANDROID UX review
VERDICT: APPROVE

## Findings

### Path to files (PASS)
- Neon panel always shows **JUNK DRAWER (all downloads)** plus full `vm.savePath` from `JunkDrawer.absolutePathHint` / live `absolutePath` on progress and LEVEL CLEAR.
- Inject status lines restate destination (`INJECTING → …`, `MAGNET → …`, `MULTI-CONN → …`, `LEVEL CLEAR — <absolute path>`).
- Path refreshes on create, resume, permission callback, and download start — no scavenger hunt for where the file went.

### Paste / share / magnet (PASS)
- On-screen **📋 PASTE** + auto-clipboard on cold start when field empty and clip looks like http/magnet/mkv.
- `ACTION_SEND` `text/plain` and `ACTION_VIEW` for `magnet` / `http` / `https` wired in manifest and `handleIntent` / `onNewIntent`.
- Paste trims quotes/angle-brackets/first line — good share-sheet hygiene.
- IME Go starts injection; START/CANCEL dual-purpose control is clear.

### Permission flow (PASS — loud, not buried)
- **GRANT STORAGE IF NEEDED** sits inside the path panel (always visible).
- Legacy READ/WRITE requests + Android 11+ `MANAGE_EXTERNAL_STORAGE` settings jump; folder is created after.
- Heavy-handed all-files access matches product goal (public Downloads/JUNK DRAWER) and is not hidden behind a buried menu.

### Arcade readability on phone (PASS)
- Scrollable cabinet column, 28dp top spacer for edge-to-edge, 220dp stage with bezel/scanlines.
- HUD row: SPEED / CONN / FILE / PHASE — dense but legible monospace neon.
- Title + blinking subtitle establish cabinet identity without crowding the stage.
- Yellow arcade heads are original cartoon noggins (spiky hair palette, red body stubs) — not licensed Simpsons assets. Syringe → inject-left-to-right metaphor reads at phone width.

### Error / status copy (PASS — playable first)
- Idle: `INSERT COIN — paste URL / YouTube / MKV / magnet`
- Fail state: `TILT — nothing faked; see error` + separate red detail line (message, not stack dump).
- Success: `LEVEL CLEAR — <path>`; cancel: `CANCELLED`; empty field: `No URL — paste something first`.
- Magnet honesty from engine surfaces as readable “no HTTP webseed” guidance — cabinet-voice friendly enough.
- Low-level `HTTP NNN` / `segment failed` can still appear as the detail line; acceptable as secondary TILT detail, not primary voice.

### Minor non-blockers (do not reject)
- `FILE` HUD truncates to 14 chars — fine for ticker; full name still in path/status when done.
- Auto-launching All Files Access settings when denied is aggressive; balanced by explicit GRANT button + path panel. Keep as-is for this product.
- No post-success “open drawer” affordance — optional polish later; path string is enough for this ship.

## Stylistic decisions (locked)
- **Loud cabinet HUD stays:** neon green path box, magenta START INJECTION, red CANCEL, cyan PASTE — no quiet Material “settings” redesign.
- **Copy retained:** INSERT COIN / TILT / LEVEL CLEAR / INJECTING / START INJECTION / GRANT STORAGE IF NEEDED.
- **Always-on path strip is mandatory UX**, not a debug leftover — keep full absolute path on screen at all times.
- **ArcadeStage density OK at 220.dp** with six heads + under-foot progress bar; do not shrink below ~200.dp without a readability recheck.
- **Original yellow heads only** — never real Simpsons IP art or names in UI strings.
- **Permission CTA label** remains shouty all-caps on the path panel; no soft “Allow access” Material tone.
- Error detail under TILT may stay technical-short; primary line stays playable arcade voice.

## Must-fix before re-review (if REJECT)
- (none)
