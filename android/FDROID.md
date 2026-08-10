# F-Droid signing for JUNK

There are **two different “F-Droid signs”**:

| Path | Who signs | You do |
|------|-----------|--------|
| **Official F-Droid.org** | F-Droid’s key | Submit metadata; they build from source |
| **Your key / private repo** | **You** (F-Droid-*style*) | `scripts/fdroid-sign.sh` |

---

## 1) Sign yourself (F-Droid-style keystore) — what you want for sideload

```bash
export ANDROID_HOME=~/Android/Sdk
export JAVA_HOME=/usr/lib/jvm/java-17-openjdk

cd ~/Projects/apps/junk/android
chmod +x scripts/fdroid-sign.sh

# Creates RSA-4096 keystore + signs a release APK
./scripts/fdroid-sign.sh
```

Output:

```text
dist/junk-fdroid-signed.apk
```

Secrets (gitignored, back up offline):

```text
android/keystore/junk-fdroid.jks
android/keystore/SECRETS.txt
android/keystore.properties
```

Install:

```bash
adb install -r ../dist/junk-fdroid-signed.apk
```

**Same keystore forever** for updates with the same `applicationId` (`dev.xstrawman.junk`). Lose the key → users must uninstall to install a new signature.

---

## 2) Official F-Droid.org listing

F-Droid **rebuilds** the app on their servers and signs with **their** key (not yours).

1. Keep the app FOSS (MIT) — already  
2. Metadata draft: `metadata/dev.xstrawman.junk.yml`  
3. Open a merge request on [fdroiddata](https://gitlab.com/fdroid/fdroiddata)  
4. Docs: https://f-droid.org/docs/Submitting_to_F-Droid_Quickstart/

You do **not** upload a pre-signed APK to official F-Droid for normal packages.

---

## 3) Private F-Droid repository (optional)

```bash
# install fdroidserver (distro package)
sudo pacman -S fdroidserver   # or pip install fdroidserver

mkdir -p ~/fdroid-junk && cd ~/fdroid-junk
fdroid init
# copy signed APK into repo/ and metadata
fdroid update --create-metadata
# serve repo/ over HTTPS; add that URL in F-Droid app → Repositories
```

Use the **same** keystore from `fdroid-sign.sh` when publishing updates.

---

## Gradle env vars (CI)

```bash
export JUNK_KEYSTORE=/path/to/junk-fdroid.jks
export JUNK_KEYSTORE_PASSWORD=…
export JUNK_KEY_ALIAS=junk
export JUNK_KEY_PASSWORD=…
./.gradle-dist/bin/gradle :app:assembleRelease
```

Or `android/keystore.properties` (see `keystore.properties.example`).
