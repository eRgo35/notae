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
  PDFs, the maintainer's local `Nutki/` folder (git-ignored), any `.notae/`
  folders from older desktop-indexer libraries, keystores (`*.jks`, `*.p12`),
  passwords, tokens and `.env` files. Test fixtures must be small, generated or
  clearly non-sensitive.
- **PDFs are never modified.** Notae must not copy, move, rename or rewrite a
  user's PDF. Changes that would break this need a design discussion first.
- **No network, no telemetry.** The Android app requests no Android permissions. Do not
  add networking or analytics dependencies.
- **Do not claim what you did not verify.** Say in the pull request what you
  ran and what you could not (for example, "not tried on a physical tablet").

## Development setup

Notae is an Android app; the host tests and the Android build run on Linux, which is the supported development machine.

1. Install the Rust toolchain with [rustup](https://rustup.rs). The repository
   pins **Rust 1.98.1** with `rustfmt` and `clippy` in
   [`rust-toolchain.toml`](rust-toolchain.toml); rustup installs it on first
   use.
2. Install the pinned cargo tools used by the gates:

   ```sh
   cargo install --locked --version 0.20.2 cargo-deny
   cargo install --locked --version 0.9.1 cargo-llvm-cov
   cargo install --locked --version 4.1.2 cargo-ndk
   ```

3. For the Android app, install a JDK (21 or newer), the Android SDK
   (platform 36, build-tools 36.0.0, NDK r28c `28.2.13676358`) with
   `ANDROID_HOME` pointing at it, and the Rust targets:

   ```sh
   rustup target add aarch64-linux-android x86_64-linux-android
   ```

   The app in `apps/notae-android` is a Gradle build (`./gradlew` there); it
   builds the Rust core with cargo-ndk and generates the Kotlin bindings with
   UniFFI.

## Project layout

| Path | What it is |
| --- | --- |
| `crates/notae-domain` | Filename convention, metadata, file identity |
| `crates/notae-index` | SQLite store, FTS5 search, rescan reconciliation |
| `crates/notae-app-core` | App state machine, stores, browse/search, rescan worker, backup, and the UniFFI API |
| `apps/notae-android` | The Gradle app: Jetpack Compose UI and the Storage Access Framework, PDF and speech platform code |

Put domain, indexing, search and screen-state logic in the host-tested crates;
keep the Kotlin app to drawing the core's snapshots and platform code. Rust
never builds display text: the core sends typed values and the app holds the
strings (`res/values*/strings.xml`). The product scope is in the
[design spec](docs/superpowers/specs/2026-10-09-notae-design.md).

## Quality gates

`scripts/check.sh` runs the gates listed in
[`docs/engineering-standards.md`](docs/engineering-standards.md), from any
directory, and stops at the first failure:

```sh
scripts/check.sh        # fast: fmt, both Clippy passes, tests
scripts/check.sh full   # fast plus rustdoc, cargo deny, coverage
                        # and the Gradle gates (lint, unit and
                        # screenshot tests, Roborazzi verify, release build)
```

Run `fast` while working and `full` before merging. `full` checks first that
`cargo-deny`, `cargo-llvm-cov`, `cargo-ndk` and the Android SDK are installed.
GitHub CI runs all of these gates, a 16 KB page-alignment check of the release
APK and an emulator smoke test, and skips changes that touch only Markdown or
`docs/`.

The opt-in pre-push hook runs `scripts/check.sh fast`. Enable it per clone with
`git config core.hooksPath scripts/hooks`; skip it once with
`git push --no-verify`.

Notes:

- Production code (libraries and binaries) must not use `unwrap`/`expect`;
  tests may. A narrow, justified invariant check is allowed if documented at
  the call site.
- `unsafe` is forbidden in the workspace, with no exceptions.
- Line coverage must stay at or above 84%.
- New dependencies must pass `cargo deny check` (licenses, advisories, sources).
- Workflows can be linted locally with `actionlint`.

## Testing guidance

- **Test behaviour, not implementation.** Prefer an integration test that goes
  through the public interface (the importer, search queries, the storage and
  operation state machines) over a test of private helpers.
- **Use real fixtures.** Prefer a small generated or non-sensitive fixture over
  mocking the code under test.
- **UI changes need behavioural UI tests.** The Compose UI is exercised on the
  JVM with Robolectric and the Compose test rules under
  `apps/notae-android/app/src/test/kotlin`, for example
  `ui/library/LibraryScreenTest.kt`. Drive the UI with clicks and key events,
  assert the visible result and the actions sent to the core, and cover
  portrait and landscape. Screens also have Roborazzi screenshots, light and
  dark; after a deliberate visual change, record them again with
  `./gradlew recordRoborazziDebug` and review the images. Run one class with,
  for example:

  ```sh
  cd apps/notae-android
  ./gradlew testDebugUnitTest --tests 'com.c2yz.notae.ui.library.LibraryScreenTest'
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
(cd apps/notae-android && ./gradlew assembleDebug)
adb -s emulator-5554 install -r apps/notae-android/app/build/outputs/apk/debug/app-debug.apk
adb -s emulator-5554 shell am start -n com.c2yz.notae/.MainActivity
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
- Open a pull request against `main` and fill in the template. The CI jobs
  (**Rust gates**, **Gradle gates**, **Emulator smoke test**) must pass; run
  `scripts/check.sh full` locally too and say so in the pull request. A newer push cancels the older run on
  the same branch.
- Update documentation and [`CHANGELOG.md`](CHANGELOG.md) (under
  `[Unreleased]`) for user-visible changes.
- Releases are cut by the maintainer by pushing a version tag, which runs
  `.github/workflows/release.yml`; see
  [`docs/android-release.md`](docs/android-release.md).
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
4. **Pull request.** Open a PR to `main`. CI runs every gate; run
   `scripts/check.sh full` locally before merging and say so in the PR.
5. **Device check.** Build with `./gradlew assembleDebug` in
   `apps/notae-android` and try it on the emulator or the Galaxy Tab S10 FE+, following the checklist in
   [`docs/android-debugging.md`](docs/android-debugging.md).
6. **Merge**, with an entry under `[Unreleased]` in
   [`CHANGELOG.md`](CHANGELOG.md).
7. **Release.** Bump the version and the lockfile, write
   `docs/release-notes/vX.Y.Z.md`, commit and push `main`, then tag and push
   the tag; the release workflow builds and publishes it
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
