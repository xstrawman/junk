# Changeset
- Summary: APK is arcade **terminal** GUI; long ASCII syringe = progress; JUNK DRAWER button + in-app `ls` panel + open system Files.
- Goals: discernible syringe; open Downloads/JUNK DRAWER; list contents in-app.
- Paths: `AsciiSyringe.kt`, `ArcadeStage.kt`, `JunkDrawerScreen.kt`, `MainActivity.kt`
- Build: assembleRelease + fdroid-sign SUCCESS; adb install when device present
- Stylistic: CRT terminal chrome, monospace only for main UX, no canvas cartoons
