## Summary

<!-- What does this change and why? Link the issue: "Fixes #N" or "Refs #N". -->

## How was it verified?

<!-- Commands you ran, emulator or device checks, screenshots (emulator captures must be labelled as such). -->
<!-- Say plainly what you could NOT verify, e.g. "not tried on a physical tablet". -->

## Checklist

Quality gates (see [CONTRIBUTING.md](https://github.com/eRgo35/notae/blob/main/CONTRIBUTING.md) and `docs/engineering-standards.md`):

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings -A clippy::unwrap_used -A clippy::expect_used`
- [ ] `cargo clippy --workspace --lib --bins -- -D warnings -D clippy::unwrap_used -D clippy::expect_used`
- [ ] `cargo test --workspace`
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
- [ ] `cargo deny check`
- [ ] `cargo llvm-cov --workspace --summary-only --fail-under-lines 84`
- [ ] `cargo run -p notae-index-cli -- --help`
- [ ] `cargo apk2 check -p notae-android --target aarch64-linux-android` (if Android code changed)

Change hygiene:

- [ ] Behaviour changes have tests (behavioural UI tests for Android UI changes) and bug fixes include a regression test
- [ ] Commits follow Conventional Commits
- [ ] `CHANGELOG.md` (`[Unreleased]`) and documentation updated for user-visible changes
- [ ] No private data, PDFs from a real library, `Nutki/` content, keystores, tokens or other secrets are included
- [ ] No new network access, telemetry or permissions (or the change is discussed in the linked issue)
