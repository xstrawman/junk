# Changeset

## Summary
- Scrapped entire Soar packaging (`packaging/soar` removed; README scrubbed).
- APK overhaul: all downloads → public `Downloads/JUNK DRAWER`.
- YouTube/stream extract via NewPipe Extractor + multi-conn HTTP.
- Storage: legacy perms + MANAGE_EXTERNAL_STORAGE prompt; path shown on HUD.
- Created `REVIEW TEAM/` agentic pre-approval roster + `.grok/AGENTS.md` gate.

## Goals / user pain fixed
- YouTube did nothing useful → NewPipe progressive/adaptive resolve then multi-conn.
- ISO path obscure → forced `…/Download/JUNK DRAWER` + on-screen path.
- Soar portion trash → deleted.
- Human not first reviewer → REVIEW TEAM protocol.

## Paths touched
- `packaging/soar/**` (deleted)
- `android/app/src/main/java/dev/xstrawman/junk/**`
- `android/app/src/main/AndroidManifest.xml`
- `android/app/build.gradle.kts`, `settings.gradle.kts`
- `REVIEW TEAM/**`, `.grok/AGENTS.md`
- `README.md`

## Build commands run + results
- `./.gradle-dist/bin/gradle :app:assembleRelease` → BUILD SUCCESSFUL
- `./scripts/fdroid-sign.sh` → `dist/junk-fdroid-signed.apk`
- `adb install -r` on device when present

## Known risks
- Magnets still webseed-best-effort (no full DHT yet) — honest errors.
- Adaptive YouTube without progressive may save video-only if no progressive stream.
- MANAGE_EXTERNAL_STORAGE is heavy-handed but required for clear public folder.

## Stylistic choices already made
- Neon cabinet HUD always shows JUNK DRAWER path.
- “TILT / LEVEL CLEAR / INJECTING” copy retained.
- Original yellow arcade heads (not Simpsons IP).
