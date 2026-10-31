#!/usr/bin/env bash
# Normalises the maintainer's release keystore into the single-entry PKCS#12 file that
# cargo-apk2 1.4.0 can use.
#
# cargo-apk2 passes only `--ks` and `--ks-pass` to apksigner: it has no key-alias or separate
# key-password option. This script copies the one named key into a new keystore whose store
# password equals its key password, so `CARGO_APK_RELEASE_KEYSTORE_PASSWORD` is that password.
#
# Usage: prepare-release-keystore.sh SOURCE_KEYSTORE DEST_P12
# Environment (never printed): NOTAE_KEYSTORE_PASSWORD, NOTAE_KEY_ALIAS, NOTAE_KEY_PASSWORD
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 SOURCE_KEYSTORE DEST_P12" >&2
  exit 2
fi
source_keystore=$1
dest=$2

missing=()
for name in NOTAE_KEYSTORE_PASSWORD NOTAE_KEY_ALIAS NOTAE_KEY_PASSWORD; do
  if [ -z "${!name:-}" ]; then
    missing+=("$name")
  fi
done
if [ "${#missing[@]}" -ne 0 ]; then
  echo "missing required environment: ${missing[*]}" >&2
  exit 1
fi
if [ ! -s "$source_keystore" ]; then
  echo "keystore file '$source_keystore' is missing or empty" >&2
  exit 1
fi

rm -f "$dest"
keytool -importkeystore -noprompt \
  -srckeystore "$source_keystore" \
  -srcstorepass:env NOTAE_KEYSTORE_PASSWORD \
  -srcalias "$NOTAE_KEY_ALIAS" \
  -srckeypass:env NOTAE_KEY_PASSWORD \
  -destkeystore "$dest" \
  -deststoretype PKCS12 \
  -destalias notae-release \
  -deststorepass:env NOTAE_KEY_PASSWORD \
  -destkeypass:env NOTAE_KEY_PASSWORD
chmod 600 "$dest"
