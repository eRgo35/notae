# Android release

Notae ships one Android artifact: a signed arm64 APK attached to the GitHub Release of a `vX.Y.Z` tag together with a `.sha256` file. `.github/workflows/release.yml` builds, signs, verifies and publishes it when the tag is pushed. Releases are signed only with the maintainer's dedicated release key; the build never falls back to the debug key.

Since 0.5.0 the app id is `com.c2yz.notae` (0.3.0 and 0.4.0 were `org.notae.android`), so 0.5.0 installs beside the old app instead of updating it.

## Where releases are built

`release.yml` runs on GitHub when a `v*` tag is pushed. It reads the release key from four repository Actions secrets:

| Secret | Meaning |
| --- | --- |
| `NOTAE_KEYSTORE_BASE64` | The keystore file (`.jks` or `.p12`), base64-encoded. |
| `NOTAE_KEYSTORE_PASSWORD` | The keystore's password. |
| `NOTAE_KEY_ALIAS` | The key's alias (`notae-release`). |
| `NOTAE_KEY_PASSWORD` | The key's password. |

Set or replace one with `gh secret set <NAME> --repo eRgo35/notae` (for the keystore: `base64 -w0 notae-release.jks | gh secret set NOTAE_KEYSTORE_BASE64 --repo eRgo35/notae`). Never commit the keystore, the passwords or the base64 text to the repository.

## One-time setup: the release key

Run these on a trusted machine, not in CI and not inside the repository. Use JDK 17 or newer.

```sh
keytool -genkeypair -v \
  -keystore notae-release.jks -storetype PKCS12 \
  -alias notae-release -keyalg RSA -keysize 4096 -validity 10000 \
  -dname "CN=Notae, O=Notae"
```

Choose strong passwords when prompted. `keytool` also accepts a separate key password for JKS keystores; PKCS12 uses one password for both.

**Back the keystore up now, before using it.** Android installs only accept an update signed with the same key, so losing `notae-release.jks` or its passwords means every user must uninstall and reinstall, losing their app data. Store at least two copies (for example an encrypted password manager attachment and an offline encrypted drive) together with the alias and both passwords.

### Expected signing certificate

The release certificate's SHA-256 fingerprint is public and pinned in `release.yml` (`NOTAE_RELEASE_CERT_SHA256`):

```text
BE:27:CD:78:17:66:C5:C2:8A:9D:B1:06:C0:7B:05:1C:EE:E4:E0:00:31:05:96:45:95:E2:A8:48:F3:71:F6:01
```

`scripts/verify-release-apk.sh` compares it with the `Signer #1 certificate SHA-256 digest` from `apksigner verify --print-certs`, and the workflow compares the AAB's certificate (`keytool -printcert -jarfile`) with it too, so a wrong or substituted keystore stops the release before any upload. Rotating the key means updating that value in a reviewed commit. Check a keystore yourself with `keytool -list -v -keystore notae-release.jks -alias notae-release`.

### How the keystore is used

The Gradle build (`apps/notae-android/app/build.gradle.kts`) signs the release build only when all four of these variables are set, and otherwise writes an unsigned `app-release-unsigned.apk`, which is what `scripts/check.sh full` and CI build:

| Variable | Meaning |
| --- | --- |
| `NOTAE_RELEASE_KEYSTORE` | Path to the keystore (`.p12` or `.jks`). |
| `NOTAE_KEYSTORE_PASSWORD` | The keystore's password. |
| `NOTAE_KEY_ALIAS` | The key's alias. |
| `NOTAE_KEY_PASSWORD` | The key's password. |

`release.yml` decodes `NOTAE_KEYSTORE_BASE64` into the runner's temporary directory, passes the path and the three other secrets to `./gradlew assembleRelease bundleRelease` only, and deletes the file after the build, whatever its outcome.

## Cutting a release

1. On `main`, bump `[workspace.package] version` in the root `Cargo.toml` and the `version = "…"` of every internal path dependency in the crates' `Cargo.toml` files, then regenerate the lockfile (`cargo update --workspace`). The Gradle build reads `versionName` from that version and derives `versionCode` as `(1 << 24) | major << 16 | minor << 8 | patch` (`apps/notae-android/buildSrc/src/main/kotlin/VersionCode.kt`, the formula every release has used), so the code rises with every semantic-version increase. Version 0.2.2 is `16777730`, 0.5.0 is `16778496`.
2. Add `docs/release-notes/vX.Y.Z.md`. It becomes the hand-written section of the GitHub Release; GitHub's generated notes are appended after it. State what was and was not verified on a physical device.
3. Run `scripts/check.sh full`, then commit (`chore(release): vX.Y.Z`), push `main` and wait for CI to pass.
4. Tag that commit and push the tag:

   ```sh
   git tag -a vX.Y.Z -m "Notae vX.Y.Z"
   git push origin vX.Y.Z
   ```

5. `release.yml` then:
   - refuses, before building anything, when a signing secret is missing, the tag is not `vMAJOR.MINOR.PATCH`, does not equal `v` plus the workspace version at the tag, has no release notes at the tag or is not on `main`;
   - runs every CI gate on the tag (`ci.yml`);
   - builds the signed release APK and AAB;
   - runs `scripts/verify-release-apk.sh`, which checks `versionName`, `versionCode`, `apksigner verify` (v2 scheme), that the certificate is not the Android debug certificate and that it equals the pinned fingerprint, and writes `notae-X.Y.Z-arm64.apk` plus `.sha256`; then checks the AAB's certificate;
   - keeps the APK, its checksum and `notae-X.Y.Z.aab` as workflow artifacts (the AAB is for a store upload; it is not attached to the release);
   - publishes the GitHub Release with the hand-written notes followed by GitHub's generated notes, and the APK and its `.sha256` attached.
6. Check the assets: `gh release view vX.Y.Z --json assets -q '.assets[].name'` lists `notae-X.Y.Z-arm64.apk` and `notae-X.Y.Z-arm64.apk.sha256`.

To retry a failed run after fixing the cause on `main`, delete the tag locally and on `origin` (and the release, if one was created), tag the fixed commit and push the tag again.

The gates include `cargo deny check` with advisories. If a new advisory lands after the last green `main` commit, the release stops at the gates; add a reasoned `deny.toml` ignore or upgrade on `main` and release a new patch version.

### Local builds

```sh
cd apps/notae-android
./gradlew assembleDebug     # app/build/outputs/apk/debug/app-debug.apk: arm64-v8a and x86_64, debug key
./gradlew assembleRelease   # app/build/outputs/apk/release/app-release-unsigned.apk: arm64-v8a, unsigned
```

The Gradle build compiles the Rust core with cargo-ndk (NDK r28c, 16 KB pages) and generates the Kotlin bindings with UniFFI; `ANDROID_HOME` (or `sdk.dir` in `local.properties`) must point at the SDK. A debug build is signed with the SDK's debug key and is not update-compatible with a release. Verify any APK with `scripts/verify-release-apk.sh APK vX.Y.Z dist`, or manually with `aapt2 dump badging`, `apksigner verify --print-certs` and `sha256sum`. The published APK is the one `release.yml` builds from the tag.

## Debug-key and release-key installs are incompatible

The Galaxy Tab S10 FE+ currently has a **debug-key** install of Notae. Android refuses to update an app with a different signing key, so installing the release-signed APK requires uninstalling the debug build first, and uninstalling **deletes the app's private data**: preferences, the SAF folder grant and the local index. The index can be rebuilt by scanning again and the PDFs are untouched, but export or back up anything you need first (note edits live only in the app index). Never `adb install -r` a release APK over the debug install; keep emulator experiments on a scratch AVD. After the move to the release key, upgrades between release builds work in place.

## Release tags and the history rewrite

On 2026-10-31, after the repository became public, its history was rewritten to remove local paths, a device serial number, a personal screenshot and agent reports, and every commit was re-signed. All commit hashes changed and the tags `v0.2.0` to `v0.3.0` were moved to the rewritten commits, keeping their names, messages and dates. `v0.2.0`, `v0.2.2` and `v0.3.0` are signed again; `v0.2.1` was never signed. The published APKs are unchanged: the rewrite touched only documentation and an unused Cargo config, never source code. Before the rewrite, `v0.2.1` pointed at a commit outside `main`, left behind when the v0.2.1 work was first re-signed on 2026-10-18; the rewrite merged it with its identical counterpart, so every release tag is now on `main`. Release tags must point at commits on `main`.
