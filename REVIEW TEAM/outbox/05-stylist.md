# STYLIST review
VERDICT: APPROVE

## Findings

### Palette — hits the cabinet
- `Arcade` (`ArcadeTheme.kt`) is real neon, not Material You sludge: near-black `Cabinet` `#0A0A18`, panel `#141428`, bezel purple `#2A1840`, then full-voltage `NeonYellow` / `NeonMagenta` / `NeonCyan` / `NeonGreen` / `NeonRed`.
- Magenta fluid (`Fluid` = magenta) is correct for “inject” — not timid blue progress bars.
- `Dim` is purple-ash `#6A6080`, not Material grey `#9E9E9D`. Acceptable for HUD labels only; never use it for primary chrome.
- XML theme (`themes.xml` / `colors.xml`) matches: window, status bar, nav bar all cabinet black; primary yellow. No light-theme bleed.

### Stage — syringe → brain-load reads
- `ArcadeStage` tells the joke in one look: giant syringe left, fluid drains with progress, needle drip while active, six original yellow noggins light up left→right with magenta pupils + cyan/magenta brain halo + yellow spark.
- Scanlines + purple stroke bezel + green foot bar keep it CRT-cabinet, not “chart widget.”
- Motion is arcade-stupid in the good way: plunger bob, head bounce on the active slot, drip pulse, flash on loaded skulls. No gentle Material fades.

### MainActivity chrome + copy
- Title `⚡ JUNK CABINET ⚡` black mono yellow — correct marquee energy.
- Tagline blinks cyan. Path HUD is always on, green border, yellow path string. Good.
- Primary CTA: `▶ START INJECTION` on magenta; cancel flips red `■ CANCEL`. Correct threat level.
- Status lexicon is cabinet, not Slack: `INSERT COIN`, `INJECTING`, `LEVEL CLEAR`, `TILT`, `PASTED — hit START INJECTION`, `CANCELLED`. Weird and clear — keep.
- Field, borders, HUD row all neon on panel; gradient background Cabinet→Bezel→Cabinet. No white surfaces.

### Not timid (locked as good enough)
- Material3 `Button` / `OutlinedTextField` shells are present but fully recolored; they do not read as default M3.
- Monospace system font stands in for pixel type without a custom face (size/weight carry it).

### Residual taste pressure (not blocking this pass)
- PHASE HUD still leaks engine words (`idle`, `downloading`, `youtube-resolve`, `magnet-resolve`). Fantasy-breaking. Map to cabinet verbs next pass if easy (`IDLE` / `CHARGE` / `INJECT` / `MAGNET` / `STREAM` / `CLEAR` / `TILT`). Not a ship-blocker: status line already carries the voice.
- Scanline alpha `0x12` is a whisper; could punch harder later without redesign.
- Corners at `8.dp` are slightly soft for pure bezel; `4.dp` or hard rects would be meaner. Secondary.

## Stylistic decisions (APPROVE — do not reopen with the user)

1. **Palette law**: Neon cyan / magenta / yellow / green / red on near-black only. No Material primary purple, no white cards, no grey chrome. `Arcade.*` is the single source of truth for Compose colors.
2. **Metaphor law**: Progress = syringe barrel empties + heads inject left→right. Do not replace with a bland linear progress bar alone. Keep original yellow cartoon heads (not Simpsons IP).
3. **Copy law**: Retain arcade bark — `INJECTING`, `TILT`, `LEVEL CLEAR`, `INSERT COIN`, `START INJECTION`, `JUNK CABINET`, `JUNK DRAWER`. Status may be weird if unambiguous.
4. **Type**: Monospace + Black/Bold for marquee and CTA; small mono for HUD. No serif, no soft body sans for chrome.
5. **HUD always shows save path** under green border; path text yellow.
6. **CTA colors**: Start = magenta on cabinet text; Cancel/running = red. Errors = neon red; success = neon green; idle status = yellow.
7. **Borders**: Magenta on stage frame, cyan on input/HUD, green on drawer path panel. Neon edges define regions — not elevation shadows.
8. **Motion**: Keep active-only anim (plunger, drip, bounce, pupil flash). Idle stage stays still enough to read as attract-mode cabinet, not a loading spinner.
9. **Characters**: Multi-hair neon tufts (blue/pink/green/magenta/cyan); loaded = magenta pupils + brain glow. Do not “refine” into realistic avatars.
10. **System chrome**: Window/status/nav stay `#0A0A18`; app name stays `JUNK`.

## Must-fix before re-review
(none — APPROVE)
