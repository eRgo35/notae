#!/usr/bin/env bash
# Builds the Notae arm64 APK on this machine and prints its path and size.
#
# Usage: build-apk.sh [release|dev]
#
#   release  (default) The shippable build. Runs exactly what the Release workflow runs:
#            `cargo apk2 build -p notae-android --target aarch64-linux-android --release`.
#            It needs the release key and never falls back to the debug key:
#              CARGO_APK_RELEASE_KEYSTORE           single-entry .p12 (see prepare-release-keystore.sh)
#              CARGO_APK_RELEASE_KEYSTORE_PASSWORD  its password
#            Output: target/release/apk/notae-android.apk
#   dev      The quick build for trying a change on a device: the `apk-dev` Cargo profile
#            (see Cargo.toml) signed with the SDK debug key. NEVER ship this APK.
#            Output: target/apk-dev/apk/notae-android.apk
#
# Environment (all optional): ANDROID_HOME / ANDROID_SDK_ROOT (default ~/Android/Sdk),
# ANDROID_NDK_ROOT (default: the r26d directory under the SDK), CARGO_TARGET_DIR.
# The JDK's Java sources are compiled as Java 17 so a newer JDK (for example 25) still produces
# class files that the bundled D8 accepts; CI uses JDK 17, which produces the same class files.
set -euo pipefail

mode=${1:-release}
case "$mode" in
  release | dev) ;;
  *) echo "usage: $0 [release|dev]" >&2; exit 2 ;;
esac

repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$repo"

export ANDROID_HOME=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}
export ANDROID_SDK_ROOT=${ANDROID_SDK_ROOT:-$ANDROID_HOME}
export ANDROID_NDK_ROOT=${ANDROID_NDK_ROOT:-$ANDROID_HOME/ndk/android-ndk-r26d}
if [ ! -d "$ANDROID_NDK_ROOT" ]; then
  echo "NDK not found at $ANDROID_NDK_ROOT; set ANDROID_NDK_ROOT (CI uses NDK r26d, 26.3.11579264)." >&2
  exit 1
fi
export JDK_JAVAC_OPTIONS=${JDK_JAVAC_OPTIONS:--source 17 -target 17}
export JAVA_SOURCE_VERSION=${JAVA_SOURCE_VERSION:-17}
export JAVA_TARGET_VERSION=${JAVA_TARGET_VERSION:-17}

target_dir=${CARGO_TARGET_DIR:-$repo/target}
if [ "$mode" = release ]; then
  if [ -z "${CARGO_APK_RELEASE_KEYSTORE:-}" ] || [ -z "${CARGO_APK_RELEASE_KEYSTORE_PASSWORD:-}" ]; then
    echo "release mode needs CARGO_APK_RELEASE_KEYSTORE and CARGO_APK_RELEASE_KEYSTORE_PASSWORD (docs/android-release.md)." >&2
    echo "For a quick debug-signed build to try on a device (not shippable), run: $0 dev" >&2
    exit 1
  fi
  cargo apk2 build -p notae-android --target aarch64-linux-android --release
  apk=$target_dir/release/apk/notae-android.apk
else
  # cargo-apk2 only falls back to the debug key for the built-in `dev` profile; a custom profile
  # reads CARGO_APK_<PROFILE>_KEYSTORE, so point it at the SDK debug key explicitly.
  export CARGO_APK_APK_DEV_KEYSTORE=${CARGO_APK_APK_DEV_KEYSTORE:-$HOME/.android/debug.keystore}
  export CARGO_APK_APK_DEV_KEYSTORE_PASSWORD=${CARGO_APK_APK_DEV_KEYSTORE_PASSWORD:-android}
  cargo apk2 build -p notae-android --target aarch64-linux-android --profile apk-dev
  apk=$target_dir/apk-dev/apk/notae-android.apk
fi

size=$(stat --format %s "$apk")
printf '%s\n%s bytes (%s)\n' "$apk" "$size" "$(numfmt --to=iec --suffix=B "$size")"
sha256sum "$apk"
if [ "$mode" = dev ]; then
  echo "dev build: debug-signed, not shippable. The shippable APK is: $0 release" >&2
fi
