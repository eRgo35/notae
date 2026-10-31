# Engineering standards

## Local Rust gates

Use Rust 1.98.1 from `rust-toolchain.toml`, including `rustfmt` and `clippy`. On Linux the host build also needs `pkg-config` and the Fontconfig development headers (`libfontconfig1-dev` on Ubuntu, `fontconfig-devel` on Fedora) because Slint's font stack links Fontconfig; the SQLite index bundles its own SQLite.

`scripts/check.sh` runs the gates from any directory and stops at the first failure:

```sh
scripts/check.sh        # fast: the first four gates below
scripts/check.sh full   # all eight, including the Android target gate
```

`full` first checks that `cargo-deny`, `cargo-llvm-cov`, `cargo-apk2` and the NDK are installed and names the install command for anything missing. On the maintainer workstation (16 threads, warm `target/`) `fast` takes about 2 minutes and `full` about 9. Run `fast` while working and `full` before merging; every release runs `full` again (see [`android-release.md`](android-release.md)). The gates, in order:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings -A clippy::unwrap_used -A clippy::expect_used
cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo deny check
cargo llvm-cov --workspace --summary-only --fail-under-lines 84
cargo apk2 check -p notae-android --target aarch64-linux-android
```

An opt-in pre-push hook runs `fast`: enable it per clone with `git config core.hooksPath scripts/hooks` and skip it once with `git push --no-verify`. No hook is mandatory.

Install the pinned tools with `cargo install --locked --version 0.20.2 cargo-deny` and `cargo install --locked --version 0.9.1 cargo-llvm-cov`. `cargo deny` reports existing transitive duplicate versions as warnings and ignores only the two unmaintained advisories with no safe upgrade; see `deny.toml`.

The measured workspace line-coverage baseline was 85.48%; the enforced floor is 84%. Test targets may use `unwrap`/`expect` for fixtures and assertions. Workspace libraries and binaries are checked separately with both Clippy lints denied; narrowly justified lock/invariant checks are documented at their callsites.

## Android target gate

Install JDK 17, the Android SDK (`platform-tools`, `platforms;android-34`, `build-tools;34.0.0`), NDK r26d (`26.3.11579264`), the `aarch64-linux-android` Rust target and `cargo-apk2` 1.4.0. `scripts/check.sh full` then runs `cargo apk2 check -p notae-android --target aarch64-linux-android`, with the same `ANDROID_HOME`, `ANDROID_NDK_ROOT` and Java 17 defaults as `scripts/build-apk.sh`. GitHub CI does not run it. Installing and visually reviewing the app on the Galaxy Tab S10 FE+ remains manual; follow [`android-debugging.md`](android-debugging.md).

## GitHub workflows

| Workflow | Trigger | Purpose |
| --- | --- | --- |
| `ci.yml` | push to `main`, pull requests, manual; skipped when only Markdown or `docs/` changed | One job: `cargo fmt --check`, both Clippy passes and `cargo test --workspace`, with `contents: read`. Superseded branch/PR runs are cancelled; `main` runs are not. It frees preinstalled runner directories and sets `CARGO_INCREMENTAL=0` and `CARGO_PROFILE_DEV_DEBUG=0` because the build fills the runner disk. It caches the Cargo registry and git sources (keyed on `Cargo.lock`) but not `target/`, for the same reason. The other gates run locally with `scripts/check.sh full`. |
| `audit.yml` | weekly, manual | `cargo deny check advisories` so new RustSec entries surface without code changes; never blocks a release. |
| `dependabot.yml` | weekly | Grouped Cargo and GitHub Actions update PRs, merged by hand. |

Releases are not built on GitHub: the maintainer runs `scripts/release.sh` (see [`android-release.md`](android-release.md)). The workflows use only first-party `actions/*`, pinned by major version. Workflows can be checked locally with `actionlint`.

## Audit summary

| Rule | Status | Severity | Evidence / remediation |
| --- | --- | --- | --- |
| BASE-001 | pass | important | `AGENTS.md` is canonical and points to these commands. |
| BASE-002 | pass | blocking | `rust-toolchain.toml` pins Rust 1.98.1, rustfmt, and clippy. |
| BASE-003 | pass | blocking | Root `Cargo.lock` is present. |
| BASE-004 | pass | blocking | `scripts/check.sh` runs the documented gates and the Android target command without hidden flags; CI runs fmt, Clippy and tests. |
| BASE-005 | pass | advisory | `cargo fmt --check` runs in `scripts/check.sh` and in CI. |
| BASE-006 | pass | important | Clippy warnings are denied across all targets. |
| BASE-007 | pass | important | Workspace and production-only Clippy passes provide static analysis. |
| BASE-008 | pass | blocking | `cargo test --workspace` exercises the behavioral suite and real fixtures. |
| BASE-009 | pass | important | `cargo llvm-cov` enforces a measured 84% line-coverage floor. |
| BASE-010 | pass | important | `cargo deny check` audits advisories, licenses, duplicate versions, and sources. |
| BASE-011 | pass | advisory | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passes. |
| BASE-012 | pass | blocking | `scripts/check.sh full` compiles Android arm64; device UI review remains manual. |
| BASE-013 | pass | important | Rescan cancellation and worker cleanup are covered by tests; worker startup errors are returned rather than panicking. |
| BASE-014 | pass | advisory | Remaining deviations below include their reason, owner, and review condition. |
| RUST-001 | pass | blocking | Exact toolchain and formatter/linter components are pinned. |
| RUST-002 | conflict | important | `apps/notae-android/Cargo.toml` retains its separate `unsafe_code = "deny"` policy because workspace `forbid` cannot be lowered at the FFI shim. Other members inherit workspace lints. Owner: Notae maintainers. Revisit when the Android FFI boundary changes. |
| RUST-003 | pass | blocking | `cargo fmt --check` runs in `scripts/check.sh` and in CI. |
| RUST-004 | pass | blocking | Clippy runs across all workspace targets; no mutually exclusive features are declared. |
| RUST-005 | pass | blocking | `cargo test --workspace` runs the host behavioral suite. |
| RUST-006 | pass | advisory | Rustdoc warning-denied build passes. |
| RUST-007 | pass | important | `deny.toml` checks advisories, licenses, sources, and bans; current transitive duplicate versions are warnings, not hidden suppressions. |
| RUST-008 | pass | important | 85.48% baseline measured; 84% floor enforced. |
| RUST-009 | pass | important | Workspace libraries and binaries deny `unwrap_used` and `expect_used`; only narrow, documented invariant exceptions remain. |
| RUST-010 | pass | blocking | Workspace forbids unsafe by default; Android uses a stricter package-level `deny` and safe core modules forbid unsafe locally. |
| COND-001 | gap | important | The workspace has domain/index/application layers, but Cargo manifests are the only direction constraint. Add a `cargo metadata`-based allowed-edge gate before introducing new internal crate dependencies. Owner: Notae maintainers. Review on the next workspace dependency change. |
| COND-002 | gap | important | Real PDF and image fixtures are tested, but there is no measured parser/render parity score. Add a fixture corpus score and floor before changing parser/rendering dependencies. Owner: Notae maintainers. Review on the next parser or renderer replacement. |
| COND-003 | not-applicable | advisory | No checked-in generated Rust or metadata artifacts require regeneration. |
| COND-004 | not-applicable | advisory | The workspace has no command-line interface; the state machines are exercised through their public APIs in integration tests. |
| COND-005 | gap | important | Android UI has no deterministic headless visual baseline; current verification requires an emulator/tablet. Keep the manual checklist in `docs/android-debugging.md`; add screenshot comparison when a stable emulator rendering environment is available. Owner: Notae maintainers. |
| COND-006 | gap | important | PDF inputs are parsed from files, but no fuzz harness exists. Add a fuzz target for PDF ingestion. Owner: Notae maintainers. Review before accepting a new parser or broader untrusted-input surface. |
| COND-007 | not-applicable | advisory | No latency or throughput requirement is claimed. |
| COND-008 | pass | important | `scripts/check.sh full` and every release cross-compile the Android arm64 target; device installation and interaction remain a manual release/debug check. |

`cargo deny` allows the MIT project license and the confirmed Slint license terms. Its remaining duplicate-version warnings come from transitive Slint dependencies; re-evaluate them during dependency upgrades.
