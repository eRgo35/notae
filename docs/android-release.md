# Android release

Notae ships one Android artifact: a signed arm64 APK attached to the GitHub Release of a `vX.Y.Z` tag together with a `.sha256` file. The maintainer builds, signs, verifies and publishes it on their own PC with `scripts/release.sh`. Releases are signed only with the maintainer's dedicated release key, which exists only on that PC and in the maintainer's backups. The build never falls back to the debug key.

## Where releases are built

Nothing on GitHub builds or signs a release. GitHub runs only the light `ci.yml` (fmt, Clippy, tests) and the weekly `audit.yml`; see [`engineering-standards.md`](engineering-standards.md). Earlier releases were built by a GitHub Actions release workflow, removed in 0.4.0 because it needed the signing key in repository secrets and about 70 CI minutes per release.

## One-time setup: the release key

Run these on a trusted machine, not in CI and not inside the repository. Use JDK 17 or newer.

```sh
keytool -genkeypair -v \
  -keystore notae-release.jks -storetype PKCS12 \
  -alias notae-release -keyalg RSA -keysize 4096 -validity 10000 \
  -dname "CN=Notae, O=Notae"
```

Choose strong passwords when prompted. `keytool` also accepts a separate key password for JKS keystores; PKCS12 uses one password for both.

**Back the keystore up now, before using it.** Android installs only accept an update signed with the same key, so losing `notae-release.jks` or its passwords means every user must uninstall and reinstall, losing their app data. Store at least two copies (for example an encrypted password manager attachment and an offline encrypted drive) together with the alias and both passwords. Never commit the keystore, the passwords or the base64 text to the repository.

### The local keystore

cargo-apk2 needs a single-entry PKCS#12 keystore whose store password equals its key password (see below). Create it once from the keystore above, outside the repository:

```sh
mkdir -p ~/.android-keys && chmod 700 ~/.android-keys
read -rs NOTAE_KEYSTORE_PASSWORD; export NOTAE_KEYSTORE_PASSWORD
read -rs NOTAE_KEY_PASSWORD; export NOTAE_KEY_PASSWORD
NOTAE_KEY_ALIAS=notae-release scripts/prepare-release-keystore.sh notae-release.jks ~/.android-keys/notae-release.p12
```

`scripts/release.sh` reads `~/.android-keys/notae-release.p12` by default; set `NOTAE_RELEASE_KEYSTORE` to use another path.

That workflow read the key from four repository secrets (`NOTAE_KEYSTORE_BASE64`, `NOTAE_KEYSTORE_PASSWORD`, `NOTAE_KEY_ALIAS`, `NOTAE_KEY_PASSWORD`). Nothing uses them any more; the maintainer may delete them with `gh secret delete <NAME> --repo eRgo35/notae`, one name at a time.

### Expected signing certificate

The release certificate's SHA-256 fingerprint is public and pinned in `scripts/release.sh` (`cert_sha256`):

```text
BE:27:CD:78:17:66:C5:C2:8A:9D:B1:06:C0:7B:05:1C:EE:E4:E0:00:31:05:96:45:95:E2:A8:48:F3:71:F6:01
```

`scripts/verify-release-apk.sh` compares it with the `Signer #1 certificate SHA-256 digest` from `apksigner verify --print-certs` and stops the release before any upload on a mismatch, so a wrong or substituted keystore cannot publish an APK. Rotating the key means updating that value in a reviewed commit. Check a keystore yourself with `keytool -list -v -keystore notae-release.jks -alias notae-release`.

### How the keystore is used

cargo-apk2 1.4.0 reads the release key from `CARGO_APK_RELEASE_KEYSTORE` (path) and `CARGO_APK_RELEASE_KEYSTORE_PASSWORD`, and passes only `--ks` and `--ks-pass` to `apksigner`. It has no key-alias or key-password option, which is why `scripts/prepare-release-keystore.sh` uses `keytool -importkeystore` to copy the one alias into a single-entry PKCS12 file whose store password is the key password. `scripts/release.sh` reads that password with a hidden prompt and passes it, with the keystore path, only to the `build-apk.sh release` command. `apps/notae-android/Cargo.toml` deliberately contains no release signing configuration: a release build without the environment variables fails instead of using the debug key.

## Cutting a release

1. On `main`, bump `[workspace.package] version` in the root `Cargo.toml` and regenerate the lockfile (`cargo update --workspace`). cargo-apk2 derives the Android `versionName` from that version and `versionCode` as `(1 << 24) | major << 16 | minor << 8 | patch`, so the code rises with every semantic-version increase; do not set either in `apps/notae-android/Cargo.toml` (cargo-apk2 rejects it). Version 0.1.0 is `16777472`, 0.2.0 is `16777728`, 0.2.2 is `16777730`.
2. Add `docs/release-notes/vX.Y.Z.md`. It becomes the hand-written section of the GitHub Release; GitHub's generated notes are appended after it. State what was and was not verified on a physical device.
3. Run `scripts/check.sh full`, then commit (`chore(release): vX.Y.Z`) and push `main`.
4. Tag that commit and push the tag:

   ```sh
   git tag -a vX.Y.Z -m "Notae vX.Y.Z"
   git push origin vX.Y.Z
   ```

5. In your own terminal (it prompts for the keystore password), run:

   ```sh
   scripts/release.sh vX.Y.Z
   ```

   It refuses, before building anything, when the tag is not `vMAJOR.MINOR.PATCH`, is not pushed or points at a different commit on `origin`, is not on `origin/main`, does not equal `v` plus the Cargo version at the tag, or has no release notes at the tag; when a GitHub Release for the tag already exists or `gh` cannot check (not installed, not logged in, offline); or when the keystore is missing. It then checks out the tag into a temporary worktree under the git-ignored `.worktrees/` and runs `scripts/check.sh full` there, so the published APK always comes from the tag. The worktree starts with an empty `target/`, so this is a cold build that needs tens of GB of disk; that is why it is not in `/tmp`, which is often RAM-backed. After the password prompt it runs `scripts/build-apk.sh release`, then `scripts/verify-release-apk.sh` checks `versionName`, `versionCode`, `apksigner verify` (v2 scheme), that the certificate is not the Android debug certificate and that it equals the pinned fingerprint, and writes `notae-X.Y.Z-arm64.apk` plus `.sha256`. Only then does `gh release create` publish both, with the hand-written notes followed by GitHub's generated notes, and print the release URL. The temporary worktree is removed on every exit, including Ctrl-C and a wrong password, and an upload interrupted part-way deletes its draft release (a published release is never deleted).
6. Check the assets: `gh release view vX.Y.Z --json assets -q '.assets[].name'` lists `notae-X.Y.Z-arm64.apk` and `notae-X.Y.Z-arm64.apk.sha256`.

The gates include `cargo deny check` with advisories. If a new advisory lands after the last green `main` commit, the release stops at the gates; add a reasoned `deny.toml` ignore or upgrade on `main` and release a new patch version.

### Local builds

One script builds the arm64 APK on this machine and prints its path, size and SHA-256:

```sh
# Shippable: the command scripts/release.sh runs (stock `release` profile, release key).
export CARGO_APK_RELEASE_KEYSTORE=/secure/path/notae-release.p12
export CARGO_APK_RELEASE_KEYSTORE_PASSWORD=...
scripts/build-apk.sh release   # -> target/release/apk/notae-android.apk

# Quick dev build: `apk-dev` profile (opt-level 1, incremental), debug key. Never ship it.
scripts/build-apk.sh dev       # -> target/apk-dev/apk/notae-android.apk
```

`release` is `cargo apk2 build -p notae-android --target aarch64-linux-android --release` and fails without the keystore variables instead of falling back to the debug key. `dev` uses the `[profile.apk-dev]` table in the root `Cargo.toml`, which inherits `release` and lowers optimisation; it does not touch the `release` profile, so the shippable artifact is unchanged. The script defaults `ANDROID_HOME` to `~/Android/Sdk` and `ANDROID_NDK_ROOT` to its `ndk/android-ndk-r26d` directory, and compiles the Java sources as Java 17 (`JDK_JAVAC_OPTIONS`), because the bundled D8 rejects the class files of a newer default JDK (JDK 25 fails with "Unsupported class file major version 69"). Both builds reuse `target/` (or `CARGO_TARGET_DIR`), so repeat builds are incremental.

Measured on the maintainer workstation (16 threads, NDK r26d, Rust 1.98.1, v0.2.2): cold `release` 114 s, `release` with nothing changed 6 s, `release` after touching `apps/notae-android/src/lib.rs` 41 s; cold `dev` 140 s, `dev` with nothing changed 15 s, `dev` after touching `lib.rs` 20 s. The point of `dev` is the edit-build-install loop (roughly half the time of `release` after a source change); a cold `dev` build is not faster. No `sccache` or `mold` is installed, and the arm64 link already uses the NDK's `lld`, so neither was added.

Create the single-entry `.p12` as in [The local keystore](#the-local-keystore). Plain `cargo apk2 build` (without `--release`) also keeps using the SDK's `~/.android/debug.keystore`.

Verify an APK with `scripts/verify-release-apk.sh target/release/apk/notae-android.apk vX.Y.Z dist`, or manually with `aapt2 dump badging`, `apksigner verify --print-certs` and `sha256sum`. A locally built APK is a convenient test build; the published APK is the one `scripts/release.sh` builds from the tag in a clean worktree.

## Debug-key and release-key installs are incompatible

The Galaxy Tab S10 FE+ currently has a **debug-key** install of Notae. Android refuses to update an app with a different signing key, so installing the release-signed APK requires uninstalling the debug build first, and uninstalling **deletes the app's private data**: preferences, the SAF folder grant and the local index. The index can be rebuilt by scanning again and the PDFs are untouched, but export or back up anything you need first (note edits live only in the app index until a sidecar is exported). Never `adb install -r` a release APK over the debug install; keep emulator experiments on a scratch AVD. After the move to the release key, upgrades between release builds work in place.

## Release tags and the commit signature rewrite

The commits of `main` after v0.2.0 (the v0.2.1 work and the first v0.2.2 fix) were first pushed unsigned and were later re-signed on 2026-10-18 (same trees, new hashes). The tag `v0.2.1` still points at the original, pre-signature commit `6f939d8`, which is no longer on `main`'s history: moving a release tag would detach it from the published, signed assets. Its tree equals the tree of the re-signed `chore(release): v0.2.1` commit, so the released APK corresponds exactly to the source on `main`. The tag `v0.2.0` is unaffected (its commits were signed from the start). Future release tags must point at commits on `main`.
