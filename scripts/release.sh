#!/usr/bin/env bash
# Builds, signs, verifies and publishes a tagged Notae release from this PC.
#
# Usage: release.sh vX.Y.Z
#
# Before running: the version bump is on origin/main, docs/release-notes/vX.Y.Z.md exists, and the
# annotated tag vX.Y.Z is pushed. The script checks all of that, then runs `check.sh full` and the
# signed release build in a temporary worktree of the tag, so the published APK always comes from
# the tag whatever this checkout contains. It prompts once for the keystore password.
#
# Environment (optional): NOTAE_RELEASE_KEYSTORE (default ~/.android-keys/notae-release.p12, the
# single-entry PKCS#12 from prepare-release-keystore.sh), ANDROID_HOME / ANDROID_SDK_ROOT.
set -euo pipefail

# The public SHA-256 of the release signing certificate (docs/android-release.md).
cert_sha256=BE:27:CD:78:17:66:C5:C2:8A:9D:B1:06:C0:7B:05:1C:EE:E4:E0:00:31:05:96:45:95:E2:A8:48:F3:71:F6:01

die() { echo "$*" >&2; exit 1; }

[ "$#" -eq 1 ] || { echo "usage: $0 vX.Y.Z" >&2; exit 2; }
tag=$1
cd "$(dirname "${BASH_SOURCE[0]}")/.."
repo=$PWD

# 1. Preflight: nothing is built or published unless every check passes. The version and the
# release notes are read from the tag, not from this checkout.
[[ $tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "tag $tag is not of the form vMAJOR.MINOR.PATCH"
version=${tag#v}
git rev-parse -q --verify "refs/tags/$tag" > /dev/null || die "tag $tag does not exist locally"
commit=$(git rev-parse "$tag^{commit}")
# An annotated tag lists its commit on the peeled `^{}` line; a lightweight tag has only one line.
remote_tags=$(git ls-remote origin "refs/tags/$tag" "refs/tags/$tag^{}")
remote_commit=$(awk -v ref="refs/tags/$tag^{}" '$2 == ref { print $1 }' <<< "$remote_tags")
[ -n "$remote_commit" ] || remote_commit=$(awk '{ print $1; exit }' <<< "$remote_tags")
[ -n "$remote_commit" ] || die "tag $tag is not on origin"
[ "$remote_commit" = "$commit" ] \
  || die "tag $tag on origin points at ${remote_commit:0:7}, the local tag at ${commit:0:7}"
# notae-android inherits [workspace.package] version from the root Cargo.toml.
cargo_version=$(git show "$commit:Cargo.toml" \
  | awk '/^\[/ { in_pkg = ($0 == "[workspace.package]") } in_pkg && /^version *=/ { gsub(/^version *= *"|"$/, ""); print; exit }')
[ "$version" = "$cargo_version" ] || die "tag $tag does not match the Cargo version ${cargo_version:-?} at the tag"
git cat-file -e "$commit:docs/release-notes/$tag.md" 2> /dev/null \
  || die "missing release notes docs/release-notes/$tag.md at the tag"
git fetch -q origin main
git merge-base --is-ancestor "$commit" origin/main || die "tag $tag is not on origin/main"
# Only "release not found" means there is no release; any other gh failure stops here.
if gh_out=$(gh release view "$tag" 2>&1); then
  die "GitHub Release $tag already exists"
fi
[[ $gh_out == *"release not found"* ]] || die "cannot check GitHub Releases for $tag: $gh_out"
keystore=${NOTAE_RELEASE_KEYSTORE:-$HOME/.android-keys/notae-release.p12}
[ -f "$keystore" ] || die "release keystore not found at $keystore (set NOTAE_RELEASE_KEYSTORE)"

# 2. Temporary worktree of the tag, removed on any exit, including Ctrl-C and a failed step. It
# lives under the git-ignored .worktrees/ on disk, not in /tmp (often RAM-backed): the cold
# build there takes tens of GB.
mkdir -p "$repo/.worktrees"
tmp=$(mktemp -d "$repo/.worktrees/release.XXXXXX")
publishing=0
cleanup() {
  # gh uploads assets to a draft before publishing it; drop that draft if publishing did not
  # finish. A published release is never deleted.
  if [ "$publishing" = 1 ] \
    && [ "$(gh release view "$tag" --json isDraft --jq .isDraft 2> /dev/null)" = true ]; then
    gh release delete "$tag" --yes > /dev/null 2>&1 || true
  fi
  git -C "$repo" worktree remove --force "$tmp/src" > /dev/null 2>&1 || true
  git -C "$repo" worktree prune
  rm -rf "$tmp"
}
trap cleanup EXIT
trap 'exit 130' INT TERM
git worktree add --quiet --detach "$tmp/src" "$tag"
src=$tmp/src

# 3. Every gate, on the tagged sources.
"$src/scripts/check.sh" full

# 4. Signed release build. The password goes only to this one command.
read -rsp "Password for $keystore: " password
echo
CARGO_APK_RELEASE_KEYSTORE=$keystore CARGO_APK_RELEASE_KEYSTORE_PASSWORD=$password \
  "$src/scripts/build-apk.sh" release
unset password
apk=${CARGO_TARGET_DIR:-$src/target}/release/apk/notae-android.apk

# 5. Version, signature scheme and the pinned release certificate; stops before any upload.
export ANDROID_HOME=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}
build_tools=$(printf '%s\n' "$ANDROID_HOME"/build-tools/* | sort -V | tail -n 1)
NOTAE_RELEASE_CERT_SHA256=$cert_sha256 \
  "$src/scripts/verify-release-apk.sh" "$apk" "$tag" "$tmp/dist" "$build_tools"

# 6. Publish.
publishing=1
gh release create "$tag" --verify-tag --title "Notae $tag" --notes-file "$src/docs/release-notes/$tag.md" --generate-notes \
  "$tmp/dist/notae-$version-arm64.apk" "$tmp/dist/notae-$version-arm64.apk.sha256"
publishing=0
gh release view "$tag" --json url --jq .url
