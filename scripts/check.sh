#!/usr/bin/env bash
# Runs the Notae quality gates from docs/engineering-standards.md on this machine.
#
# Usage: check.sh [fast|full]
#
#   fast  (default) cargo fmt, both Clippy passes and the test suite. Run it while working;
#         the opt-in pre-push hook (scripts/hooks/pre-push) runs it too.
#   full  fast, then rustdoc, cargo deny, the coverage floor, the CLI smoke and the Android
#         arm64 compile check. Run it before merging and in every release (scripts/release.sh).
#
# Stops at the first failing gate. Acts on the repository this script lives in, whatever the cwd.
# Environment for `full` (all optional): ANDROID_HOME / ANDROID_SDK_ROOT (default ~/Android/Sdk),
# ANDROID_NDK_ROOT (default: the r26d directory under the SDK).
set -euo pipefail

mode=${1:-fast}
case "$mode" in
  fast | full) ;;
  *) echo "usage: $0 [fast|full]" >&2; exit 2 ;;
esac

# Not `git rev-parse --show-toplevel`: hooks in a linked worktree run with GIT_DIR set, which
# makes git report the cwd as the top level.
cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [ "$mode" = full ]; then
  # Same defaults as build-apk.sh.
  export ANDROID_HOME=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}
  export ANDROID_SDK_ROOT=${ANDROID_SDK_ROOT:-$ANDROID_HOME}
  export ANDROID_NDK_ROOT=${ANDROID_NDK_ROOT:-$ANDROID_HOME/ndk/android-ndk-r26d}
  export JDK_JAVAC_OPTIONS=${JDK_JAVAC_OPTIONS:--source 17 -target 17}
  export JAVA_SOURCE_VERSION=${JAVA_SOURCE_VERSION:-17}
  export JAVA_TARGET_VERSION=${JAVA_TARGET_VERSION:-17}

  missing=0
  for tool in cargo-deny:0.20.2 cargo-llvm-cov:0.9.1 cargo-apk2:1.4.0; do
    name=${tool%%:*}
    if ! command -v "$name" > /dev/null; then
      echo "missing: $name (install: cargo install --locked --version ${tool#*:} $name)" >&2
      missing=1
    fi
  done
  if [ ! -d "$ANDROID_NDK_ROOT" ]; then
    echo "missing: NDK at $ANDROID_NDK_ROOT (set ANDROID_NDK_ROOT)" >&2
    missing=1
  fi
  [ "$missing" = 0 ] || exit 1
fi

passed=0
start=$SECONDS
gate() {
  local t=$SECONDS
  echo "==> $*"
  "$@"
  echo "    $((SECONDS - t))s"
  passed=$((passed + 1))
}

gate cargo fmt --check
gate cargo clippy --workspace --all-targets -- -D warnings -A clippy::unwrap_used -A clippy::expect_used
gate cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
gate cargo test --workspace

if [ "$mode" = full ]; then
  gate env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
  gate cargo deny check
  # Reuses target/llvm-cov-target between runs; no `cargo clean` needed locally.
  gate cargo llvm-cov --workspace --summary-only --fail-under-lines 84
  gate cargo run -p notae-index-cli -- --help
  gate cargo apk2 check -p notae-android --target aarch64-linux-android
fi

echo "All $passed gates passed in $((SECONDS - start))s"
