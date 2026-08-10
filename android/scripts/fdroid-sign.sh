#!/usr/bin/env bash
# F-Droid-style APK signing for Junk (your own keystore / personal F-Droid repo).
#
# Official F-Droid.org: they rebuild from source and sign with *their* key.
# This script is for:
#   • sideload / GitHub releases signed by you
#   • a private F-Droid binary repository you host
#
# Usage:
#   ./scripts/fdroid-sign.sh              # build release + sign
#   ./scripts/fdroid-sign.sh --init-only  # only create keystore
#   ./scripts/fdroid-sign.sh path/to.apk  # sign an existing APK
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$ROOT/.." && pwd)"
KEYDIR="${JUNK_KEYSTORE_DIR:-$ROOT/keystore}"
KEYSTORE="${JUNK_KEYSTORE:-$KEYDIR/junk-fdroid.jks}"
ALIAS="${JUNK_KEY_ALIAS:-junk}"
PROPS="$ROOT/keystore.properties"
DIST="$REPO_ROOT/dist"
ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-17-openjdk}"
export JAVA_HOME PATH="$JAVA_HOME/bin:$PATH"

# Prefer newest apksigner
APKSIGNER="$(ls -1 "$ANDROID_HOME"/build-tools/*/apksigner 2>/dev/null | sort -V | tail -1 || true)"
if [[ -z "$APKSIGNER" ]]; then
  echo "error: apksigner not found under \$ANDROID_HOME/build-tools" >&2
  exit 1
fi

mkdir -p "$KEYDIR" "$DIST"

# --- create keystore if missing (F-Droid-compatible RSA 4096, 30y) ---
if [[ ! -f "$KEYSTORE" ]]; then
  echo "==> Creating F-Droid-style keystore at $KEYSTORE"
  echo "    (keep this file + passwords private; losing it means you can't update the same app id)"
  STOREPASS="${JUNK_KEYSTORE_PASSWORD:-$(openssl rand -base64 24)}"
  KEYPASS="${JUNK_KEY_PASSWORD:-$STOREPASS}"

  keytool -genkeypair \
    -keystore "$KEYSTORE" \
    -alias "$ALIAS" \
    -keyalg RSA \
    -keysize 4096 \
    -validity 10950 \
    -storepass "$STOREPASS" \
    -keypass "$KEYPASS" \
    -dname "CN=Junk F-Droid, OU=xstrawman, O=junk, L=Internet, ST=NA, C=US"

  umask 077
  cat > "$PROPS" <<EOF
storeFile=$KEYSTORE
storePassword=$STOREPASS
keyAlias=$ALIAS
keyPassword=$KEYPASS
EOF
  chmod 600 "$PROPS" "$KEYSTORE"
  echo "==> Wrote $PROPS (gitignored). Back this up offline."
  # Also dump a one-time secrets file for humans
  cat > "$KEYDIR/SECRETS.txt" <<EOF
# DO NOT COMMIT — back up offline
KEYSTORE=$KEYSTORE
ALIAS=$ALIAS
STORE_PASSWORD=$STOREPASS
KEY_PASSWORD=$KEYPASS
EOF
  chmod 600 "$KEYDIR/SECRETS.txt"
fi

# Load passwords
if [[ -f "$PROPS" ]]; then
  # shellcheck disable=SC1090
  set -a
  # properties file is key=value
  STOREPASS="$(grep '^storePassword=' "$PROPS" | cut -d= -f2-)"
  KEYPASS="$(grep '^keyPassword=' "$PROPS" | cut -d= -f2-)"
  ALIAS="$(grep '^keyAlias=' "$PROPS" | cut -d= -f2-)"
  KEYSTORE="$(grep '^storeFile=' "$PROPS" | cut -d= -f2-)"
  set +a
fi
STOREPASS="${JUNK_KEYSTORE_PASSWORD:-${STOREPASS:?missing store password}}"
KEYPASS="${JUNK_KEY_PASSWORD:-${KEYPASS:-$STOREPASS}}"
ALIAS="${JUNK_KEY_ALIAS:-$ALIAS}"

if [[ "${1:-}" == "--init-only" ]]; then
  echo "Keystore ready: $KEYSTORE"
  exit 0
fi

# --- build unsigned/signed release ---
APK_IN="${1:-}"
if [[ -z "$APK_IN" || "$APK_IN" == "--build" ]]; then
  echo "==> Building release APK"
  cd "$ROOT"
  if [[ -x "$ROOT/.gradle-dist/bin/gradle" ]]; then
    GRADLE="$ROOT/.gradle-dist/bin/gradle"
  else
    GRADLE=gradle
  fi
  # Export for Gradle signingConfigs
  export JUNK_KEYSTORE="$KEYSTORE"
  export JUNK_KEYSTORE_PASSWORD="$STOREPASS"
  export JUNK_KEY_ALIAS="$ALIAS"
  export JUNK_KEY_PASSWORD="$KEYPASS"
  "$GRADLE" :app:assembleRelease
  APK_IN="$ROOT/app/build/outputs/apk/release/app-release.apk"
  # Gradle may produce -unsigned if signing failed
  if [[ ! -f "$APK_IN" ]]; then
    APK_IN="$ROOT/app/build/outputs/apk/release/app-release-unsigned.apk"
  fi
fi

if [[ ! -f "$APK_IN" ]]; then
  echo "error: APK not found: $APK_IN" >&2
  echo "Build debug first or pass a path to an APK." >&2
  exit 1
fi

OUT="$DIST/junk-fdroid-signed.apk"
echo "==> Signing with apksigner (v1+v2+v3) → $OUT"
rm -f "$OUT"
"$APKSIGNER" sign \
  --ks "$KEYSTORE" \
  --ks-key-alias "$ALIAS" \
  --ks-pass "pass:$STOREPASS" \
  --key-pass "pass:$KEYPASS" \
  --v1-signing-enabled true \
  --v2-signing-enabled true \
  --v3-signing-enabled true \
  --out "$OUT" \
  "$APK_IN"

"$APKSIGNER" verify --verbose "$OUT"
echo
echo "✓ F-Droid-style signed APK:"
echo "  $OUT"
ls -lh "$OUT"
echo
echo "Install: adb install -r \"$OUT\""
echo "Fingerprint (compare on updates):"
keytool -list -v -keystore "$KEYSTORE" -alias "$ALIAS" -storepass "$STOREPASS" 2>/dev/null \
  | grep -E 'SHA-256|SHA1:' | head -4
