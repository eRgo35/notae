#!/usr/bin/env bash
# Attaches the universal APK that Google Play signed to the draft GitHub Release of TAG, then
# publishes the release (docs/android-release.md).
#
# Verifies the APK with scripts/verify-release-apk.sh against Google Play's app signing certificate,
# read from NOTAE_APP_SIGNING_CERT_SHA256 in .github/workflows/release.yml, which writes
# notae-X.Y.Z-arm64.apk and its .sha256 into dist/; uploads both and publishes the draft. Refuses
# unless the release exists and is still a draft.
#
# Usage: attach-play-apk.sh TAG APK
# Needs gh (signed in) and the build tools verify-release-apk.sh needs (ANDROID_HOME).
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 TAG APK" >&2
  exit 2
fi
tag=$1
apk=$(realpath "$2")
cd "$(dirname "${BASH_SOURCE[0]}")/.."

cert=$(sed -n 's/^ *NOTAE_APP_SIGNING_CERT_SHA256: *//p' .github/workflows/release.yml)
if [ -z "$cert" ]; then
  echo "NOTAE_APP_SIGNING_CERT_SHA256 not found in .github/workflows/release.yml" >&2
  exit 1
fi

draft=$(gh release view "$tag" --json isDraft --jq .isDraft) || {
  echo "no GitHub Release for $tag; release.yml creates it from the pushed tag" >&2
  exit 1
}
if [ "$draft" != true ]; then
  echo "the GitHub Release for $tag is already published" >&2
  exit 1
fi

NOTAE_RELEASE_CERT_SHA256=$cert scripts/verify-release-apk.sh "$apk" "$tag" dist

name="notae-${tag#v}-arm64.apk"
gh release upload "$tag" "dist/$name" "dist/$name.sha256"
gh release edit "$tag" --draft=false
gh release view "$tag" --json assets --jq '.assets[].name'
