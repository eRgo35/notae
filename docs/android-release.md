# Android release

Notae ships one Android artifact: a signed arm64 APK built by the **Release** workflow from a `vX.Y.Z` tag and attached to the GitHub Release together with a `.sha256` file. Releases are signed only with the maintainer's dedicated release key, which exists only in repository secrets and in the maintainer's backups. The build never falls back to the debug key.

## Workflows

| File | Trigger | What it does |
| --- | --- | --- |
| `.github/workflows/ci.yml` | push to `main`, pull requests, `workflow_dispatch`, and `workflow_call` | Job **Rust quality gates**: every gate in [`engineering-standards.md`](engineering-standards.md). Job **Android arm64 compile**: `cargo apk2 check` for `aarch64-linux-android`. Runs with `contents: read`; a newer run on a branch or PR cancels the older one, runs on `main` always finish. |
| `.github/workflows/release.yml` | push of a `v*` tag, or `workflow_dispatch` with an existing tag | **preflight** (signing secrets present, tag commit is on `main`, tag equals the Cargo version), **gates** (the CI workflow above on the tagged commit), **release** (build, sign, verify, upload artifacts, attach to the GitHub Release; the only job with `contents: write`). |
| `.github/workflows/audit.yml` | weekly (Monday 05:17 UTC) and `workflow_dispatch` | `cargo deny check advisories`, so new RustSec advisories surface without a code change. It is a separate workflow: a failure reports the advisory but does not block commits, pull requests or releases. |
| `.github/dependabot.yml` | weekly | Grouped Cargo (minor and patch) and GitHub Actions update pull requests. Nothing is merged automatically. |

The release workflow reruns the full CI gates, including `cargo deny check` with advisories. If a new advisory lands between the green `main` commit and the tag, the release stops at **gates**; add a reasoned `deny.toml` ignore or upgrade, push, and tag again.

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

### Repository Actions secrets

Set these four secrets (Settings, Secrets and variables, Actions, or the `gh` commands below):

| Secret | Value |
| --- | --- |
| `NOTAE_KEYSTORE_BASE64` | The keystore file, base64-encoded on one line |
| `NOTAE_KEYSTORE_PASSWORD` | The keystore (store) password |
| `NOTAE_KEY_ALIAS` | The key alias, `notae-release` in the example above |
| `NOTAE_KEY_PASSWORD` | The key password (equal to the store password for PKCS12) |

```sh
base64 --wrap=0 notae-release.jks | gh secret set NOTAE_KEYSTORE_BASE64 --repo eRgo35/notae
gh secret set NOTAE_KEYSTORE_PASSWORD --repo eRgo35/notae
gh secret set NOTAE_KEY_ALIAS --repo eRgo35/notae
gh secret set NOTAE_KEY_PASSWORD --repo eRgo35/notae
```

`gh secret set` without `--body` prompts for the value, which keeps passwords out of shell history. Check the names with `gh secret list --repo eRgo35/notae` (values are never shown).

### Expected signing certificate

The release certificate's SHA-256 fingerprint is public and pinned in `.github/workflows/release.yml` (`NOTAE_RELEASE_CERT_SHA256`):

```text
BE:27:CD:78:17:66:C5:C2:8A:9D:B1:06:C0:7B:05:1C:EE:E4:E0:00:31:05:96:45:95:E2:A8:48:F3:71:F6:01
```

`.github/scripts/verify-release-apk.sh` compares it with the `Signer #1 certificate SHA-256 digest` from `apksigner verify --print-certs` and fails the release on any mismatch, so a wrong or substituted secret cannot publish an APK. Rotating the key means updating that value in a reviewed commit. Check a keystore yourself with `keytool -list -v -keystore notae-release.jks -alias notae-release`.

### How the secrets are used

cargo-apk2 1.4.0 reads the release key from `CARGO_APK_RELEASE_KEYSTORE` (path) and `CARGO_APK_RELEASE_KEYSTORE_PASSWORD`, and passes only `--ks` and `--ks-pass` to `apksigner`. It has no key-alias or key-password option, so the workflow first runs `.github/scripts/prepare-release-keystore.sh`, which uses `keytool -importkeystore` to copy the one alias named by `NOTAE_KEY_ALIAS` into a single-entry PKCS12 file under `$RUNNER_TEMP` whose store password is `NOTAE_KEY_PASSWORD`. The build step receives that file and password through its environment only. The decoded and normalised keystores are deleted in an `always()` step. `apps/notae-android/Cargo.toml` deliberately contains no release signing configuration: a release build without the environment variables fails instead of using the debug key.

## Cutting a release

1. On `main`, bump `[workspace.package] version` in the root `Cargo.toml` and regenerate the lockfile (`cargo update --workspace`). cargo-apk2 derives the Android `versionName` from that version and `versionCode` as `(1 << 24) | major << 16 | minor << 8 | patch`, so the code rises with every semantic-version increase; do not set either in `apps/notae-android/Cargo.toml` (cargo-apk2 rejects it). Version 0.1.0 is `16777472`, 0.2.0 is `16777728`, 0.2.2 is `16777730`.
2. Add `docs/release-notes/vX.Y.Z.md`. It becomes the hand-written section of the GitHub Release; GitHub's generated notes are appended after it. State what was and was not verified on a physical device.
3. Commit, push `main`, and wait for the **CI** run on that exact commit to pass both jobs.
4. Tag that commit and push the tag:

   ```sh
   git tag -a vX.Y.Z -m "Notae vX.Y.Z"
   git push origin vX.Y.Z
   ```

5. The **Release** workflow runs. It fails early with an error naming the missing secrets if any are absent, if the tag commit is not on `main`, or if the tag does not equal `v` plus the Cargo version. After the gates pass it builds `cargo apk2 build -p notae-android --target aarch64-linux-android --release`, then `.github/scripts/verify-release-apk.sh` checks `versionName`, `versionCode`, `apksigner verify` (v2 scheme) and that the certificate is not the Android debug certificate and equals the pinned fingerprint, and writes `notae-X.Y.Z-arm64.apk` plus `.sha256`. It uploads both as workflow artifacts and attaches them to the GitHub Release, creating the release (hand-written notes plus generated notes) only if it does not exist.
6. To attach the APK to a tag that was pushed before the secrets existed, add the secrets and run the workflow manually: `gh workflow run release.yml --repo eRgo35/notae -f tag=vX.Y.Z`. Re-running replaces the assets of the same name.

### Local builds

One script builds the arm64 APK on this machine and prints its path, size and SHA-256:

```sh
# Shippable: the exact command the Release workflow runs (stock `release` profile, release key).
export CARGO_APK_RELEASE_KEYSTORE=/secure/path/notae-release.p12
export CARGO_APK_RELEASE_KEYSTORE_PASSWORD=...
.github/scripts/build-apk.sh release   # -> target/release/apk/notae-android.apk

# Quick dev build: `apk-dev` profile (opt-level 1, incremental), debug key. Never ship it.
.github/scripts/build-apk.sh dev       # -> target/apk-dev/apk/notae-android.apk
```

`release` is `cargo apk2 build -p notae-android --target aarch64-linux-android --release` and fails without the keystore variables instead of falling back to the debug key. `dev` uses the `[profile.apk-dev]` table in the root `Cargo.toml`, which inherits `release` and lowers optimisation; it does not touch the `release` profile, so the shippable artifact is unchanged. The script defaults `ANDROID_HOME` to `~/Android/Sdk` and `ANDROID_NDK_ROOT` to its `ndk/android-ndk-r26d` directory, and compiles the Java sources as Java 17 (`JDK_JAVAC_OPTIONS`), because the bundled D8 rejects the class files of a newer default JDK (JDK 25 fails with "Unsupported class file major version 69"). Both builds reuse `target/` (or `CARGO_TARGET_DIR`), so repeat builds are incremental.

Measured on the maintainer workstation (16 threads, NDK r26d, Rust 1.98.1, v0.2.2): cold `release` 114 s, `release` with nothing changed 6 s, `release` after touching `apps/notae-android/src/lib.rs` 41 s; cold `dev` 140 s, `dev` with nothing changed 15 s, `dev` after touching `lib.rs` 20 s. The point of `dev` is the edit-build-install loop (roughly half the time of `release` after a source change); a cold `dev` build is not faster. No `sccache` or `mold` is installed, and the arm64 link already uses the NDK's `lld`, so neither was added.

Create the single-entry `.p12` with `NOTAE_KEYSTORE_PASSWORD=... NOTAE_KEY_ALIAS=... NOTAE_KEY_PASSWORD=... .github/scripts/prepare-release-keystore.sh notae-release.jks notae-release.p12`. Plain `cargo apk2 build` (without `--release`) also keeps using the SDK's `~/.android/debug.keystore`.

Verify an APK with `.github/scripts/verify-release-apk.sh target/release/apk/notae-android.apk vX.Y.Z dist`, or manually with `aapt2 dump badging`, `apksigner verify --print-certs` and `sha256sum`. A locally built APK is a convenient test build; the published APK is the one the Release workflow builds from the tag.

## Debug-key and release-key installs are incompatible

The Galaxy Tab S10 FE+ currently has a **debug-key** install of Notae. Android refuses to update an app with a different signing key, so installing the release-signed APK requires uninstalling the debug build first, and uninstalling **deletes the app's private data**: preferences, the SAF folder grant and the local index. The index can be rebuilt by scanning again and the PDFs are untouched, but export or back up anything you need first (note edits live only in the app index until a sidecar is exported). Never `adb install -r` a release APK over the debug install; keep emulator experiments on a scratch AVD. After the move to the release key, upgrades between release builds work in place.
