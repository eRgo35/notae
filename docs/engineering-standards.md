# Engineering standards

## Local Rust gates

Use Rust 1.98.1 from `rust-toolchain.toml`, including `rustfmt` and `clippy`. On Linux, install `pkg-config`, Tesseract development headers, and Leptonica development headers. Fedora/RHEL hosts without system pkg-config files can follow [`crates/notae-ocr/README.md`](../crates/notae-ocr/README.md) and set `PKG_CONFIG_PATH=/tmp/tesseract-pkgconfig` for Cargo commands.

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings -A clippy::unwrap_used -A clippy::expect_used
cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo deny check
cargo llvm-cov --workspace --summary-only --fail-under-lines 84
cargo run -p notae-index-cli -- --help
```

Install the pinned tools with `cargo install --locked --version 0.20.2 cargo-deny` and `cargo install --locked --version 0.9.1 cargo-llvm-cov`. `cargo deny` reports existing transitive duplicate versions as warnings and ignores only the two unmaintained advisories with no safe upgrade; see `deny.toml`.

The measured workspace line-coverage baseline was 85.48%; the enforced floor is 84%. Test targets may use `unwrap`/`expect` for fixtures and assertions. Workspace libraries and binaries are checked separately with both Clippy lints denied; narrowly justified lock/invariant checks are documented at their callsites.

## Android target gate

Install JDK 17, Android SDK command-line tools, NDK r26d, and `cargo-apk2` 1.4.0. Then run:

```sh
cargo apk2 check -p notae-android --target aarch64-linux-android
```

CI runs the same arm64 target check. Installing and visually reviewing the app on the Galaxy Tab S10 FE+ remains manual; follow [`android-debugging.md`](android-debugging.md).

## Audit summary

| Rule | Status | Severity | Evidence / remediation |
| --- | --- | --- | --- |
| BASE-001 | pass | important | `AGENTS.md` is canonical and points to these commands. |
| BASE-002 | pass | blocking | `rust-toolchain.toml` pins Rust 1.98.1, rustfmt, and clippy. |
| BASE-003 | pass | blocking | Root `Cargo.lock` is present. |
| BASE-004 | pass | blocking | `.github/workflows/ci.yml` runs the documented host gates and Android target command without hidden flags. |
| BASE-005 | pass | advisory | `cargo fmt --check` runs locally and in CI. |
| BASE-006 | pass | important | Clippy warnings are denied across all targets. |
| BASE-007 | pass | important | Workspace and production-only Clippy passes provide static analysis. |
| BASE-008 | pass | blocking | `cargo test --workspace` exercises the behavioral suite and real fixtures. |
| BASE-009 | pass | important | `cargo llvm-cov` enforces a measured 84% line-coverage floor. |
| BASE-010 | pass | important | `cargo deny check` audits advisories, licenses, duplicate versions, and sources. |
| BASE-011 | pass | advisory | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` passes. |
| BASE-012 | pass | blocking | CI smokes CLI `--help` and compiles Android arm64; device UI review remains manual. |
| BASE-013 | pass | important | Rescan cancellation and worker cleanup are covered by tests; worker startup errors are returned rather than panicking. |
| BASE-014 | pass | advisory | Remaining deviations below include their reason, owner, and review condition. |
| RUST-001 | pass | blocking | Exact toolchain and formatter/linter components are pinned. |
| RUST-002 | conflict | important | `apps/notae-android/Cargo.toml` retains its separate `unsafe_code = "deny"` policy because workspace `forbid` cannot be lowered at the FFI shim. Other members inherit workspace lints. Owner: Notae maintainers. Revisit when the Android FFI boundary changes. |
| RUST-003 | pass | blocking | `cargo fmt --check` runs locally and in CI. |
| RUST-004 | pass | blocking | Clippy runs across all workspace targets; no mutually exclusive features are declared. |
| RUST-005 | pass | blocking | `cargo test --workspace` runs the host behavioral suite. |
| RUST-006 | pass | advisory | Rustdoc warning-denied build passes. |
| RUST-007 | pass | important | `deny.toml` checks advisories, licenses, sources, and bans; current transitive duplicate versions are warnings, not hidden suppressions. |
| RUST-008 | pass | important | 85.48% baseline measured; 84% floor enforced. |
| RUST-009 | pass | important | Workspace libraries and binaries deny `unwrap_used` and `expect_used`; only narrow, documented invariant exceptions remain. |
| RUST-010 | pass | blocking | Workspace forbids unsafe by default; Android uses a stricter package-level `deny` and safe core modules forbid unsafe locally. |
| COND-001 | gap | important | The workspace has domain/index/OCR/application layers, but Cargo manifests are the only direction constraint. Add a `cargo metadata`-based allowed-edge gate before introducing new internal crate dependencies. Owner: Notae maintainers. Review on the next workspace dependency change. |
| COND-002 | gap | important | Real PDF, image, and sidecar fixtures are tested, but there is no measured parser/render parity score. Add a fixture corpus score and floor before changing parser/rendering dependencies. Owner: Notae maintainers. Review on the next parser or renderer replacement. |
| COND-003 | not-applicable | advisory | No checked-in generated Rust or metadata artifacts require regeneration. |
| COND-004 | pass | blocking | CLI argument/exit-code integration tests and the CLI `--help` smoke exercise the public interface. |
| COND-005 | gap | important | Android UI has no deterministic headless visual baseline; current verification requires an emulator/tablet. Keep the manual checklist in `docs/android-debugging.md`; add screenshot comparison when a stable emulator rendering environment is available. Owner: Notae maintainers. |
| COND-006 | gap | important | PDF and sidecar inputs are parsed from files, but no fuzz harness exists. Add fuzz targets for sidecar validation and PDF ingestion. Owner: Notae maintainers. Review before accepting a new parser or broader untrusted-input surface. |
| COND-007 | not-applicable | advisory | No latency or throughput requirement is claimed. |
| COND-008 | pass | important | CI cross-compiles the Android arm64 target; device installation and interaction remain a manual release/debug check. |

`cargo deny` allows the MIT project license and the confirmed Slint license terms. Its remaining duplicate-version warnings come from transitive OCR, PDF, and Slint dependencies; re-evaluate them during dependency upgrades.
