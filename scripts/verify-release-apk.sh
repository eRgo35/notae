#!/usr/bin/env bash
# Verifies a release APK against its tag and copies it, with a SHA-256 file, into DIST_DIR.
#
# Checks: versionName equals the tag without `v`; versionCode equals the value the Gradle build
# derives from the semantic version ((1 << 24) | major << 16 | minor << 8 | patch, see
# apps/notae-android/buildSrc/src/main/kotlin/VersionCode.kt); the APK verifies with
# apksigner and the v2 scheme; it is not signed with the Android debug key; and, when
# NOTAE_RELEASE_CERT_SHA256 is set, the signing certificate's SHA-256 fingerprint equals it
# (hex, with or without colons, any case). .github/workflows/release.yml always sets it.
#
# Usage: verify-release-apk.sh APK TAG DIST_DIR [BUILD_TOOLS_DIR]
# BUILD_TOOLS_DIR defaults to $ANDROID_HOME/build-tools/36.0.0 and must hold aapt2 and apksigner.
set -euo pipefail

if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
  echo "usage: $0 APK TAG DIST_DIR [BUILD_TOOLS_DIR]" >&2
  exit 2
fi
apk=$1
tag=$2
dist=$3
build_tools=${4:-${ANDROID_HOME:?set ANDROID_HOME or pass BUILD_TOOLS_DIR}/build-tools/36.0.0}

case "$tag" in
  v[0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "::error::tag '$tag' is not of the form vMAJOR.MINOR.PATCH" >&2; exit 1 ;;
esac
version=${tag#v}
IFS=. read -r major minor patch <<< "${version%%[-+]*}"
expected_code=$(( (1 << 24) | (major << 16) | (minor << 8) | patch ))

badging=$("$build_tools/aapt2" dump badging "$apk")
echo "$badging" | head -n 1
if ! echo "$badging" | head -n 1 | grep -q "versionName='${version}'"; then
  echo "::error::APK versionName is not ${version}" >&2
  exit 1
fi
if ! echo "$badging" | head -n 1 | grep -q "versionCode='${expected_code}'"; then
  echo "::error::APK versionCode is not ${expected_code}" >&2
  exit 1
fi

report=$(mktemp)
trap 'rm -f "$report"' EXIT
"$build_tools/apksigner" verify --verbose --print-certs "$apk" | tee "$report"
if ! grep -q 'Verified using v2 scheme (APK Signature Scheme v2): true' "$report"; then
  echo "::error::APK is not v2-signed" >&2
  exit 1
fi
if grep -qi 'CN=Android Debug' "$report"; then
  echo "::error::APK is signed with the Android debug key" >&2
  exit 1
fi

signers=$(grep -c '^Signer #[0-9]* certificate SHA-256 digest:' "$report" || true)
if [ "$signers" -ne 1 ]; then
  echo "::error::expected exactly one APK signer, found $signers" >&2
  exit 1
fi
actual=$(sed -n 's/^Signer #1 certificate SHA-256 digest: //p' "$report" | tr -d ': \r' | tr 'A-F' 'a-f')
echo "Signing certificate SHA-256: $actual"
if [ -n "${NOTAE_RELEASE_CERT_SHA256:-}" ]; then
  expected=$(printf '%s' "$NOTAE_RELEASE_CERT_SHA256" | tr -d ': ' | tr 'A-F' 'a-f')
  if [ "$actual" != "$expected" ]; then
    echo "::error::APK signing certificate $actual does not match the expected release certificate $expected" >&2
    exit 1
  fi
  echo "Signing certificate matches the expected release certificate."
fi

mkdir -p "$dist"
name="notae-${version}-arm64.apk"
cp "$apk" "$dist/$name"
(cd "$dist" && sha256sum "$name" | tee "$name.sha256")
