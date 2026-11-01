#!/usr/bin/env bash
# Smoke-tests a Notae debug APK on a booted emulator, as CI's emulator job runs it: installs the
# APK on a fresh app state, launches it, chooses a fixture folder holding one PDF in the system
# folder picker, waits for the library and opens Sets. It drives the UI through `uiautomator dump`
# and `input tap`, so it needs only adb.
#
# Usage: emulator-smoke.sh APK
#
# Uses the one device adb sees; set ANDROID_SERIAL to choose another. Use a scratch emulator: the
# script uninstalls Notae first and leaves the fixture folder in the shared storage root.
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 APK" >&2
  exit 2
fi
apk=$1
package=com.c2yz.notae
# Starts with a digit so it sorts first in the picker's storage root, without scrolling.
folder="0 Notae smoke"
dump=/sdcard/notae-smoke-ui.xml

# Prints "x y", the centre of the first node whose text or content description is $1, or nothing.
# Case is ignored: the system picker shows some buttons in capitals ("USE THIS FOLDER").
find_node() {
  adb shell uiautomator dump "$dump" > /dev/null 2>&1 || return 0
  adb exec-out cat "$dump" | tr '>' '\n' \
    | grep -F -i -e "text=\"$1\"" -e "content-desc=\"$1\"" | head -n 1 \
    | sed -n 's/.*bounds="\[\([0-9]*\),\([0-9]*\)\]\[\([0-9]*\),\([0-9]*\)\]".*/\1 \2 \3 \4/p' \
    | awk '{ print int(($1 + $3) / 2), int(($2 + $4) / 2) }'
}

# Waits up to 60 s for a node named $1 and prints its centre; fails the test without one.
wait_for() {
  local at
  for _ in $(seq 1 30); do
    at=$(find_node "$1")
    if [ -n "$at" ]; then
      echo "$at"
      return 0
    fi
    sleep 2
  done
  echo "smoke test failed: \"$1\" did not appear" >&2
  return 1
}

tap() {
  local at
  at=$(wait_for "$1")
  echo "tap \"$1\" at $at"
  # shellcheck disable=SC2086 # "x y" splits into the two coordinates.
  adb shell input tap $at
}

echo "==> fixture folder /sdcard/$folder"
pdf=$(mktemp)
trap 'rm -f "$pdf"' EXIT
printf '%s\n' '%PDF-1.4' '1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj' \
  '2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj' \
  '3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] >> endobj' \
  'trailer << /Root 1 0 R >>' '%%EOF' > "$pdf"
adb shell mkdir -p "\"/sdcard/$folder\""
adb push "$pdf" "/sdcard/$folder/1 - Smoke.pdf" > /dev/null

echo "==> install and launch"
adb uninstall "$package" > /dev/null 2>&1 || true
adb install "$apk"
adb shell am start -W -n "$package/.MainActivity"

echo "==> choose the fixture folder"
tap "Choose score folder"
tap "$folder"
tap "Use this folder"
tap "Allow"

echo "==> library, then Sets"
wait_for "A little space for your scores" > /dev/null
tap "Sets"
wait_for "No sets yet" > /dev/null

if [ -z "$(adb shell pidof "$package" | tr -d '\r')" ]; then
  echo "smoke test failed: $package is not running" >&2
  exit 1
fi
adb shell rm -f "$dump"
echo "Smoke test passed: installed, launched, chose a folder and opened Sets."
