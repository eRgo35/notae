#!/usr/bin/env bash
# Runs the Notae quality gates from docs/engineering-standards.md on this machine.
#
# Usage: check.sh [fast|full]
#
#   fast  (default) cargo fmt, both Clippy passes and the test suite. Run it while working;
#         the opt-in pre-push hook (scripts/hooks/pre-push) runs it too.
#   full  fast, then rustdoc, cargo deny, the coverage floor, the third-party license notices and
#         the Gradle gates of the Android app (lint, unit and screenshot tests, Roborazzi verify, the
#         release build). Run it before merging.
#
# Stops at the first failing gate. Acts on the repository this script lives in, whatever the cwd.
# Environment for `full` (optional): ANDROID_HOME / ANDROID_SDK_ROOT (default ~/Android/Sdk). The
# NDK is the `ndkVersion` in apps/notae-android/app/build.gradle.kts, under the SDK.
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
  export ANDROID_HOME=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}

  missing=0
  for tool in cargo-deny:0.20.2 cargo-llvm-cov:0.9.1 cargo-ndk:4.1.2 cargo-about:0.9.3; do
    name=${tool%%:*}
    if ! command -v "$name" > /dev/null; then
      # cargo-about's binary needs its `cli` feature.
      features=""
      [ "$name" = cargo-about ] && features=" --features cli"
      echo "missing: $name (install: cargo install --locked --version ${tool#*:}$features $name)" >&2
      missing=1
    fi
  done
  if [ ! -d "$ANDROID_HOME" ]; then
    echo "missing: Android SDK at $ANDROID_HOME (set ANDROID_HOME)" >&2
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

# The license notices the app shows (about.toml, about.hbs) must match what cargo-about writes
# for the current Cargo.lock.
licenses() {
  local asset=apps/notae-android/app/src/main/assets/third_party_licenses.txt fresh
  fresh=$(mktemp)
  cargo fetch --locked
  cargo about generate -c about.toml -m crates/notae-app-core/Cargo.toml --no-default-features \
    --locked --offline --fail about.hbs -o "$fresh"
  if ! cmp -s "$fresh" "$asset"; then
    diff -u "$asset" "$fresh" | head -n 40 >&2 || true
    rm -f "$fresh"
    echo "$asset is out of date; regenerate it (docs/engineering-standards.md)" >&2
    return 1
  fi
  rm -f "$fresh"
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
  gate licenses
  gate apps/notae-android/gradlew -p apps/notae-android lint testDebugUnitTest verifyRoborazziDebug assembleRelease
fi

echo "All $passed gates passed in $((SECONDS - start))s"
