# Engineering standards

## Local gates

Use Rust 1.98.1 from `rust-toolchain.toml`, including `rustfmt` and `clippy`. The SQLite index bundles its own SQLite, so the host build needs no system libraries.

`scripts/check.sh` runs the gates from any directory and stops at the first failure:

```sh
scripts/check.sh        # fast: the first four gates below
scripts/check.sh full   # all eight, including the Gradle gates of the Android app
```

`full` first checks that `cargo-deny`, `cargo-llvm-cov`, `cargo-ndk` and the Android SDK are installed and names the install command for anything missing. Run `fast` while working and `full` before merging. The gates, in order:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings -A clippy::unwrap_used -A clippy::expect_used
cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo deny check
cargo llvm-cov --workspace --summary-only --fail-under-lines 84
apps/notae-android/gradlew -p apps/notae-android lint testDebugUnitTest verifyRoborazziDebug assembleRelease
```

An opt-in pre-push hook runs `fast`: enable it per clone with `git config core.hooksPath scripts/hooks` and skip it once with `git push --no-verify`. No hook is mandatory.

Install the pinned tools with `cargo install --locked --version 0.20.2 cargo-deny`, `cargo install --locked --version 0.9.1 cargo-llvm-cov` and `cargo install --locked --version 4.1.2 cargo-ndk`. `cargo deny` reports existing transitive duplicate versions as warnings and ignores only the unmaintained-crate advisories with no safe upgrade, each with its reason; see `deny.toml`.

The measured workspace line-coverage baseline was 85.48% (89.72% in 0.5.0); the enforced floor is 84%. Test targets may use `unwrap`/`expect` for fixtures and assertions. Workspace libraries and binaries are checked separately with both Clippy lints denied; narrowly justified lock/invariant checks are documented at their callsites.

## Android gates

The Android app in `apps/notae-android` is a Gradle build (Kotlin, Jetpack Compose). Install a JDK (21 or newer; CI uses Temurin 25), the Android SDK (`platforms;android-36`, `build-tools;36.0.0`, `ndk;28.2.13676358`), the `aarch64-linux-android` and `x86_64-linux-android` Rust targets and `cargo-ndk`. `ANDROID_HOME` defaults to `~/Android/Sdk` in `scripts/check.sh`. The Gradle build compiles the Rust core with cargo-ndk (16 KB pages) and generates its Kotlin bindings with UniFFI.

- `lint`: Android lint of the app.
- `testDebugUnitTest`: the JVM unit tests, Robolectric UI tests and accessibility checks. They load a host build of the Rust core through JNA.
- `verifyRoborazziDebug`: compares the Roborazzi screenshots with the baselines in `apps/notae-android/app/src/test/screenshots`. After a deliberate visual change, record new baselines with `./gradlew recordRoborazziDebug` and review the images in the diff.
- `assembleRelease`: the arm64 release build. Without the signing variables it writes an unsigned APK (see [`android-release.md`](android-release.md)).

CI also checks that the release APK is 16 KB-aligned (`zipalign -c -P 16` and every `LOAD` segment of its native libraries) and runs an emulator smoke test, `scripts/emulator-smoke.sh`: install, launch, choose a fixture folder in the system picker, open Sets. Installing and reviewing the app on the Galaxy Tab S10 FE+ stays manual; follow [`android-debugging.md`](android-debugging.md).

## GitHub workflows

| Workflow | Trigger | Purpose |
| --- | --- | --- |
| `ci.yml` | push to `main`, pull requests, manual, and called by `release.yml`; push and PR runs skip changes to only Markdown or `docs/` | Three jobs with `contents: read`. **Rust gates**: the first seven gates above. **Gradle gates**: the Gradle command above plus `assembleDebug`, then the 16 KB check; it uploads the debug APK, and the Roborazzi diffs when a gate fails. **Emulator smoke test**: boots an API 35 tablet AVD (KVM enabled through a udev rule) and runs `scripts/emulator-smoke.sh` on that APK. Superseded branch/PR runs are cancelled; `main` runs are not. The Rust job frees preinstalled runner directories and sets `CARGO_INCREMENTAL=0` and `CARGO_PROFILE_DEV_DEBUG=0`; the Cargo registry and git sources are cached (keyed on `Cargo.lock`), `target/` is not. |
| `release.yml` | push of a `v*` tag | Checks the tag (`vX.Y.Z`, equal to the workspace version, on `main`, with release notes) and the signing secrets, runs `ci.yml` on the tag, builds the signed APK and AAB, verifies them against the pinned certificate and publishes the GitHub Release with the APK. See [`android-release.md`](android-release.md). |
| `audit.yml` | weekly, manual | `cargo deny check advisories` so new RustSec entries surface without code changes; never blocks a release. |
| `dependabot.yml` | weekly | Grouped Cargo and GitHub Actions update PRs, merged by hand. |

The workflows use only first-party `actions/*`, pinned by major version. Workflows can be checked locally with `actionlint`.

## Audit summary

| Rule | Status | Severity | Evidence / remediation |
| --- | --- | --- | --- |
| BASE-001 | pass | important | `AGENTS.md` is canonical and points to these commands. |
| BASE-002 | pass | blocking | `rust-toolchain.toml` pins Rust 1.98.1, rustfmt, and clippy. |
| BASE-003 | pass | blocking | Root `Cargo.lock` is present. |
| BASE-004 | pass | blocking | `scripts/check.sh` runs the documented gates, including the Gradle gates, without hidden flags; CI runs all of them plus the 16 KB check and the emulator smoke test. |
| BASE-005 | pass | advisory | `cargo fmt --check` runs in `scripts/check.sh` and in CI. |
| BASE-006 | pass | important | Clippy warnings are denied across all targets. |
| BASE-007 | pass | important | Workspace and production-only Clippy passes provide static analysis. |
| BASE-008 | pass | blocking | `cargo test --workspace` exercises the behavioral suite and real fixtures. |
| BASE-009 | pass | important | `cargo llvm-cov` enforces a measured 84% line-coverage floor. |
| BASE-010 | pass | important | `cargo deny check` audits advisories, licenses, duplicate versions, and sources. |
| BASE-011 | pass | advisory | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passes. |
| BASE-012 | pass | blocking | `scripts/check.sh full` and CI build the arm64 release APK; CI also runs an emulator smoke test. Device UI review remains manual. |
| BASE-013 | pass | important | Rescan cancellation and worker cleanup are covered by tests; worker startup errors are returned rather than panicking. |
| BASE-014 | pass | advisory | Remaining deviations below include their reason, owner, and review condition. |
| RUST-001 | pass | blocking | Exact toolchain and formatter/linter components are pinned. |
| RUST-002 | pass | important | Resolved in 0.5.0: every workspace member inherits the workspace lints. The old UI crate and its `unsafe_code = "deny"` override for the `android_main` FFI shim are gone; the Kotlin app reaches the core through UniFFI's generated bindings, which need no `unsafe` in Notae's code. |
| RUST-003 | pass | blocking | `cargo fmt --check` runs in `scripts/check.sh` and in CI. |
| RUST-004 | pass | blocking | Clippy runs across all workspace targets; no mutually exclusive features are declared. |
| RUST-005 | pass | blocking | `cargo test --workspace` runs the host behavioral suite. |
| RUST-006 | pass | advisory | Rustdoc warning-denied build passes. |
| RUST-007 | pass | important | `deny.toml` checks advisories, licenses, sources, and bans; current transitive duplicate versions are warnings, not hidden suppressions. |
| RUST-008 | pass | important | 85.48% baseline measured; 84% floor enforced. |
| RUST-009 | pass | important | Workspace libraries and binaries deny `unwrap_used` and `expect_used`; only narrow, documented invariant exceptions remain. |
| RUST-010 | pass | blocking | The workspace forbids `unsafe` and no member overrides it; Notae has no `unsafe` code. |
| COND-001 | gap | important | The workspace has domain/index/application layers, but Cargo manifests are the only direction constraint. Add a `cargo metadata`-based allowed-edge gate before introducing new internal crate dependencies. Owner: Notae maintainers. Review on the next workspace dependency change. |
| COND-002 | gap | important | Real PDF and image fixtures are tested, but there is no measured parser/render parity score. Add a fixture corpus score and floor before changing parser/rendering dependencies. Owner: Notae maintainers. Review on the next parser or renderer replacement. |
| COND-003 | not-applicable | advisory | No checked-in generated Rust or metadata artifacts require regeneration. |
| COND-004 | not-applicable | advisory | The workspace has no command-line interface; the state machines are exercised through their public APIs in integration tests. |
| COND-005 | pass | important | Closed in 0.5.0: Roborazzi screenshots of every screen, light and dark, are compared with checked-in baselines (`verifyRoborazziDebug`) in `scripts/check.sh full` and CI, headless under Robolectric. The tablet check stays manual. |
| COND-006 | gap | important | PDF inputs are parsed from files, but no fuzz harness exists. Add a fuzz target for PDF ingestion. Owner: Notae maintainers. Review before accepting a new parser or broader untrusted-input surface. |
| COND-007 | not-applicable | advisory | No latency or throughput requirement is claimed. |
| COND-008 | pass | important | `scripts/check.sh full`, CI and every release build the arm64 release APK, and CI installs and drives a debug build on an emulator; installation and interaction on the tablet remain a manual release check. |

`cargo deny` allows the MIT project license, the permissive licenses of the Rust dependencies and MPL-2.0 for UniFFI (owner-approved; file-level copyleft, used unmodified). UniFFI's generated scaffolding compiles under `#![forbid(unsafe_code)]`, so `notae-app-core` needs no exception.
