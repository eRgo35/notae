# Android Emulator Verification and GitHub Release Design

## Goal

Exercise Notae on an Android tablet emulator in local development and CI, and publish a signed, downloadable arm64 APK as a GitHub Release when a version tag is pushed.

## Approved design

The maintainer approved this design on 2026-10-16:

- Add a GitHub Actions Android emulator job using an API 35 tablet profile and x86_64 system image. Build/install an x86_64 test APK and exercise application launch plus the emulator-verifiable UI paths. Preserve the existing host Rust tests and Android arm64 compile gate.
- Add a release job for pushed `v*` tags. It must depend on the host quality, arm64 compile, and emulator jobs; build a release APK for `aarch64-linux-android`; sign it with the maintainer's release keystore supplied through GitHub Actions secrets; and publish the APK and a SHA-256 checksum as GitHub Release assets.
- Merge only reviewed, verified work to `main`.

The release trigger is tag-based rather than every push to `main`, to avoid publishing releases for unversioned commits. A release is created only when a matching tag is pushed and all required jobs pass.

## Constraints and boundaries

- Keep Rust pinned to 1.98.1, Android min SDK 23, target SDK 34, and the existing NDK r26d / cargo-apk2 toolchain unless a demonstrated compatibility requirement forces a reviewed change.
- Release signing material and passwords must never enter the repository or logs. The workflow consumes GitHub Actions secrets; maintainers must configure those secrets before pushing a release tag.
- The current package metadata points at `/home/mike/.android/debug.keystore`; remove the machine-specific release signing configuration and replace it with a CI-safe mechanism supported by the pinned cargo-apk2 version.
- The app's configured Android build target is arm64, while the available local tablet AVD is x86_64. Emulator builds must produce an x86_64 APK without changing the arm64 release target or weakening the existing arm64 gate.
- Do not claim emulator verification proves Samsung One UI, physical tablet rendering, or TalkBack acceptance. Keep those explicit device-only checks open.
- Do not close milestone issues solely because the emulator or compile job passes.

## Emulator acceptance

The emulator job must boot a tablet-shaped API 35 AVD, install the x86_64 APK, launch `org.notae.android/.NotaeActivity`, and fail on installation failure, failure to reach the expected foreground activity, app process termination, or fatal application logs. It must exercise the agreed app paths through an observable interaction mechanism and preserve useful failure evidence (logcat and screenshots) as CI artifacts. Reuse the host-generated UI behavioral tests for assertions that are more reliable at the Slint component seam; do not replace them with source-text checks or brittle coordinate-only assertions.

The local AVD `notae-api35-tablet` is API 35 x86_64 at 2560×1600. Its initial cold-boot attempt reported ADB offline and exited with code 1 before a boot-complete signal; the failure cause remains undiagnosed. CI must use a reproducible runner-managed AVD rather than rely on this local process. Local emulator boot should still be retried and diagnosed as part of verification.

## CI and release behavior

- Existing pull request and branch CI gates remain in force.
- The emulator test runs on pull requests and pushes to branches, using an Android emulator runner compatible with GitHub-hosted Linux runners.
- The release APK builder also runs on pull requests and branch pushes with a generated, disposable CI-only keystore, so the signed release build is exercised before merge without exposing the maintainer's release secret. Pushed `v*` tags and explicit `workflow_dispatch` dry runs use the configured maintainer keystore; only a pushed `v*` tag may publish.
- The publishing job is conditional on `refs/tags/v*` and requires successful `rust-quality`, `android-target`, emulator, and signed release-build jobs. Branch and pull-request release-build validation uses an ephemeral test key and cannot publish.
- Release builds are arm64, signed using a dedicated maintainer-owned keystore loaded from repository secrets into a temporary file with restrictive permissions, then removed in an `always()` cleanup step.
- The workflow creates a GitHub Release for the pushed tag and attaches the APK and checksum. It uses the repository-provided `GITHUB_TOKEN` with only the `contents: write` permission needed to publish the release.
- Release job logs must not print secret values, encoded keystore contents, or passwords. Missing signing secrets fail before the release is created.
- No release tag is created as part of this change unless separately requested after verification.

## Verification

- Run the existing documented host quality gates and Android arm64 compile gate.
- Run the emulator workflow locally against the available AVD where possible; verify install, launch, representative navigation/actions, and crash-free logs. Also run the same workflow on GitHub after changes are pushed.
- Verify the release APK package name, arm64 ABI, version, signing certificate, and checksum. Install the x86_64 emulator build to verify runtime behavior; do not require an arm64 APK to install on the x86_64 AVD.
- On pull requests and branch pushes, verify the signed arm64 release build using a disposable CI key before merge. After the workflow is present on the default branch, dispatch a real-key dry run and verify it does not publish.
- Exercise tag release automation with a non-production test tag or an equivalent release dry run before publishing the intended version.
- Review the workflow permissions, secret handling, artifact names, and release behavior before merging to `main`.
