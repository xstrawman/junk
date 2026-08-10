# JUNK Android APK — Arcade Cabinet

90s **Simpsons Arcade–era vibe** (original cartoon line — not Fox IP):

- Paste **HTTP(S)** / **.mkv** / **magnet:** links  
- Multi-connection hypersonic HTTP downloads  
- Magnet: best-effort webseed resolve (full DHT/libtorrent next)  
- Progress = **syringe** emptying into a **line of yellow cartoon noggins**

## Build

```bash
export ANDROID_HOME=~/Android/Sdk
export JAVA_HOME=/usr/lib/jvm/java-17-openjdk   # or your JDK 17+

cd android
# install compile SDK if needed:
# sdkmanager "platforms;android-35" "build-tools;35.0.0"

./gradlew :app:assembleDebug
# APK:
# app/build/outputs/apk/debug/app-debug.apk
```

Install on device/emulator:

```bash
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

## Use

1. Open **JUNK CABINET**  
2. Tap **PASTE** (or share a link into the app)  
3. **START INJECTION**  
4. Watch the cabinet meter + brain-load line  
5. Files land in app-specific `Downloads/junk/`

## Notes

- Characters are **original** yellow arcade heads (not The Simpsons).  
- Magnets without HTTP webseeds need native libtorrent in a follow-up.  
- Streaming sites (YouTube) are CLI/desktop via yt-dlp for now.
