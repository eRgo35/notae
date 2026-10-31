# Contributing to Notae

Thanks for helping. Notae is a small, local-first project, so contributions
that keep it simple, private and well-tested are the most welcome. This guide
covers the development setup, the quality gates, the development workflow and
the conventions the repository follows.

By contributing you agree that your work is released under the
[MIT License](LICENSE). Security problems go to [`SECURITY.md`](SECURITY.md),
**not** to a public issue.

## Ground rules

- **Never commit private data.** That includes personal score libraries and
  PDFs, the maintainer's local `Nutki/` folder (git-ignored), the `.notae/`
  sidecars generated from real libraries, keystores (`*.jks`, `*.p12`),
  passwords, tokens and `.env` files. Test fixtures must be small, generated or
  clearly non-sensitive.
- **PDFs are never modified.** Notae must not copy, move, rename or rewrite a
  user's PDF. Changes that would break this need a design discussion first.
- **No network, no telemetry.** The Android app declares no permissions. Do not
  add networking or analytics dependencies.
- **Do not claim what you did not verify.** Say in the pull request what you
  ran and what you could not (for example, "not tried on a physical tablet").

## Development setup

You need a Linux machine. macOS and Windows are not supported for the indexer.

1. Install the Rust toolchain with [rustup](https://rustup.rs). The repository
   pins **Rust 1.98.1** with `rustfmt` and `clippy` in
   [`rust-toolchain.toml`](rust-toolchain.toml); rustup installs it on first
   use.
2. Install the native libraries used by the OCR and PDF stack. On Ubuntu:

   ```sh
   sudo apt-get install pkg-config libtesseract-dev libleptonica-dev \
       libfontconfig1-dev tesseract-ocr-eng
   ```

   CI sets `TESSDATA_PREFIX=/usr/share/tesseract-ocr/5/tessdata` so subprocess
   tests find the language data. On Fedora/RHEL hosts without system
   `pkg-config` files for Tesseract, follow
   [`crates/notae-ocr/README.md`](crates/notae-ocr/README.md) and set
   `PKG_CONFIG_PATH`.
3. Install the pinned cargo tools used by the gates:

   ```sh
   cargo install --locked --version 0.20.2 cargo-deny
   cargo install --locked --version 0.9.1 cargo-llvm-cov
   ```

4. For Android work, install JDK 17, the Android SDK (platform 34, build-tools
   34.0.0), **NDK r26d** (`26.3.11579264`), the Rust target and `cargo-apk2`:

   ```sh
   rustup target add aarch64-linux-android
   cargo install --locked --version 1.4.0 cargo-apk2
   export ANDROID_NDK_ROOT="$ANDROID_HOME/ndk/26.3.11579264"
   ```

   If your JDK is newer than 17, compile the Java adapter as Java 17; the
   exact environment variables are in
   [`docs/android-debugging.md`](docs/android-debugging.md).

## Project layout

| Path | What it is |
| --- | --- |
| `crates/notae-domain` | Filename convention, metadata, file identity |
| `crates/notae-index` | SQLite store, FTS5 search, sidecar format, importer |
| `crates/notae-ocr` | Tesseract-backed OCR provider |
| `apps/notae-index` | The `notae-index` command-line indexer |
| `apps/notae-android` | Slint UI, Storage Access Framework adapter, Java shim |

Put domain, indexing and search logic in the host-tested crates; keep the
Android crate to UI and platform glue. The product scope is in the
[design spec](docs/superpowers/specs/2026-10-09-notae-design.md).

## Quality gates

`scripts/check.sh` runs the gates listed in
[`docs/engineering-standards.md`](docs/engineering-standards.md), from any
directory, and stops at the first failure:

```sh
scripts/check.sh        # fast: fmt, both Clippy passes, tests
scripts/check.sh full   # fast plus rustdoc, cargo deny, coverage, CLI smoke,
                        # and the Android target check
```

Run `fast` while working and `full` before merging. `full` checks first that
`cargo-deny`, `cargo-llvm-cov`, `cargo-apk2` and the NDK are installed. GitHub
CI runs only fmt, Clippy and the tests, and skips changes that touch only
Markdown or `docs/`; everything else is checked locally.

The opt-in pre-push hook runs `scripts/check.sh fast`. Enable it per clone with
`git config core.hooksPath scripts/hooks`; skip it once with
`git push --no-verify`.

Notes:

- Production code (libraries and binaries) must not use `unwrap`/`expect`;
  tests may. A narrow, justified invariant check is allowed if documented at
  the call site.
- `unsafe` is forbidden in the workspace. The only exception is the Android
  entry-point and JNI shim under `apps/notae-android`, which uses a stricter
  package-level `deny` and allows `unsafe` locally with a reason.
- Line coverage must stay at or above 84%.
- New dependencies must pass `cargo deny check` (licenses, advisories, sources).
- Workflows can be linted locally with `actionlint`.

## Testing guidance

- **Test behaviour, not implementation.** Prefer an integration test that goes
  through the public interface (CLI arguments and exit codes, sidecar output,
  the importer, search queries) over a test of private helpers.
- **Use real fixtures.** The indexer tests use real PDF fixtures under
  `apps/notae-index/tests/fixtures/`; add a small fixture rather than mocking
  the PDF parser.
- **UI changes need behavioural UI tests.** The Android UI is exercised on the
  host with Slint's testing backend: see
  [`apps/notae-android/tests/support/mod.rs`](apps/notae-android/tests/support/mod.rs)
  (`app()` / `app_sized(width, height)`) and tests such as `library_ui.rs`,
  `headers_ui.rs` and `search_input.rs`. Drive the generated UI with pointer and
  key events, assert the visible result and callbacks, and cover both compact
  and wide window sizes. Run a single target with, for example:

  ```sh
  cargo test -p notae-android --test library_ui
  ```

- **Write the failing test first** for bug fixes, and keep it as the
  regression test.
- Host tests are not device acceptance. Say so in your pull request when a
  change affects layout, input methods, motion or accessibility.

## Running on the Android emulator

Use a **scratch** emulator, never a device that holds your real library.
[`docs/android-debugging.md`](docs/android-debugging.md) has the full guide,
including AVD creation. In short:

```sh
cargo apk2 build -p notae-android --target x86_64-linux-android
adb -s emulator-5554 install -r target/debug/apk/notae-android.apk
adb -s emulator-5554 shell am start -n org.notae.android/.NotaeActivity
```

Always pass an explicit `-s <serial>`. Check `adb devices -l` first and do not
run `adb install`, `pm clear` or `adb uninstall` against a physical tablet that
contains real data. If you attach screenshots, capture them from an emulator
with a non-sensitive fixture library and label them as emulator captures.

Debug builds are signed with your SDK debug key and cannot be updated in place
by the official release APK (and vice versa); see
[`docs/android-release.md`](docs/android-release.md).

## Branches, commits and pull requests

- Branch from `main`. Use a short prefix and an issue number where there is
  one: `feat/…`, `fix/…`, `docs/…`, `chore/…` (for example
  `fix/32-hardware-keyboard-polish`).
- Use [Conventional Commits](https://www.conventionalcommits.org/):
  `<type>(optional-scope): imperative summary`, for example
  `fix(search): preserve result panel height`. Common types: `feat`, `fix`,
  `docs`, `test`, `refactor`, `perf`, `build`, `ci`, `chore`. Reference issues
  in the body (`Refs #12`, `Fixes #12`).
- Keep commits small and focused; do not mix refactors with behaviour changes.
  No hook is mandatory, so run the gates yourself (or enable the opt-in hook).
- Open a pull request against `main` and fill in the template. The CI job
  (**fmt, clippy, tests**) must pass, and `scripts/check.sh full` must pass
  locally; say so in the pull request. A newer push cancels the older run on
  the same branch.
- Update documentation and [`CHANGELOG.md`](CHANGELOG.md) (under
  `[Unreleased]`) for user-visible changes.
- Releases are cut by the maintainer from a version tag with
  `scripts/release.sh`; see [`docs/android-release.md`](docs/android-release.md).
  Contributors do not need to bump versions.

## Development workflow

Notae has one maintainer, working with Claude. Each change goes through these
steps:

1. **Issue.** Open or pick an issue, give it a triage label and a milestone.
2. **Branch.** Branch from `main` (`feat/…`, `fix/…`) or use a worktree under
   `.worktrees/`. Multi-step work gets a plan in `docs/superpowers/plans/`. UI
   or architecture ideas can be mocked as a private Claude Artifact before any
   code.
3. **Build test-first.** Write the failing test, then the code. Run
   `scripts/check.sh fast` while working, or let the opt-in hook run it on
   push.
4. **Pull request.** Open a PR to `main`. The light CI runs; run
   `scripts/check.sh full` locally before merging and say so in the PR.
5. **Device check.** Build with `scripts/build-apk.sh dev` and try it on the
   emulator or the Galaxy Tab S10 FE+, following the checklist in
   [`docs/android-debugging.md`](docs/android-debugging.md).
6. **Merge**, with an entry under `[Unreleased]` in
   [`CHANGELOG.md`](CHANGELOG.md).
7. **Release.** Bump the version and the lockfile, write
   `docs/release-notes/vX.Y.Z.md`, commit and push `main`, tag and push the
   tag, then run `scripts/release.sh vX.Y.Z`
   ([`docs/android-release.md`](docs/android-release.md)).
8. **Close issues** only after the change is verified on a device.

## Issues and labels

Bugs and ideas live in
[GitHub Issues](https://github.com/eRgo35/notae/issues); use the issue forms.
The maintainer triages with these labels (see
[`docs/agents/triage-labels.md`](docs/agents/triage-labels.md)):

| Label | Meaning |
| --- | --- |
| `needs-triage` | The maintainer still has to evaluate it |
| `needs-info` | Waiting on the reporter for more information |
| `ready-for-agent` | Fully specified; can be implemented without further discussion |
| `ready-for-human` | Needs a human to implement |
| `wontfix` | Will not be actioned |

The usual GitHub labels (`bug`, `enhancement`, `documentation`,
`good first issue`, `help wanted`, `accessibility`) are also used. Planned work
is grouped into [milestones](https://github.com/eRgo35/notae/milestones).

When reporting a bug, include the Notae version (Settings, About), the device
and Android version, whether you ran on a physical tablet or an emulator, and
the steps to reproduce. For a crash, attach the relevant `adb logcat -b crash`
block (see [`docs/android-debugging.md`](docs/android-debugging.md)) but remove
file names, `content://` URIs and other personal identifiers first.

Maintainer-facing conventions for the issue tracker and domain vocabulary are
in [`docs/agents/`](docs/agents/) and [`AGENTS.md`](AGENTS.md).
