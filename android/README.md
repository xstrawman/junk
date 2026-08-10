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

## F-Droid-style signing (your key)

```bash
chmod +x scripts/fdroid-sign.sh
./scripts/fdroid-sign.sh
# → ../dist/junk-fdroid-signed.apk
```

Creates an RSA-4096 keystore (gitignored). Full notes: **[FDROID.md](./FDROID.md)**.

Official F-Droid.org uses *their* key after a source rebuild — metadata draft: `../metadata/dev.xstrawman.junk.yml`.

## Use

1. Open **JUNK CABINET**  
2. Tap **PASTE** (or share a link into the app)  
3. **START INJECTION**  
4. Watch the cabinet meter + brain-load line  
5. Files always land in public **`Download/JUNK DRAWER`** (shown on screen).

## Notes

- Characters are **original** yellow arcade heads (not The Simpsons).  
- Magnets without HTTP webseeds need native libtorrent in a follow-up.  
- YouTube / extractable streams use **NewPipe Extractor** + multi-conn (progressive preferred).
