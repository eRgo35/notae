# Changelog

All notable changes to Notae are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and Notae adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
The hand-written release notes for each version live in
[`docs/release-notes/`](docs/release-notes/).

## [Unreleased]

Notae 0.5.0 is a new Android app: the same library, search, favorites and sets,
rebuilt in Jetpack Compose over the Rust core.

### Changed

- **New application id `com.c2yz.notae`.** 0.5.0 installs beside 0.3.0 and
  0.4.0 instead of updating them, and starts with no library. To keep your
  favorites and sets, use **Export sets** in the old app's Settings, then
  **Import from a previous Notae** on 0.5.0's first screen (or **Import sets**
  in Settings later). Pick the score folder again; the PDFs are rescanned.
  Only favorites and sets move across: edited details (number, title,
  composer, tags) and the Recent list are not carried over, so keep the old
  app installed until you have checked the new one.
- The UI is Jetpack Compose (Android 8.0, API 26, or newer; tablets only), and
  the Rust core is a library the app calls through UniFFI. The app is built
  with Gradle and cargo-ndk, with 16 KB-aligned native code.
- GitHub CI runs every Rust gate, the Gradle gates (lint, unit and screenshot
  tests, Roborazzi verify, the release build and a 16 KB alignment check) and
  an emulator smoke test. `scripts/check.sh full` runs the Gradle gates too.
- Releases are built, signed and verified by `.github/workflows/release.yml`
  from a pushed `vX.Y.Z` tag, which attaches the APK to the GitHub Release.

### Removed

- The desktop indexer (`notae-index` CLI) and its Tesseract OCR, and the
  sidecar import of its `.notae/manifest.json`. Scanned PDFs without a text
  layer are found by file name only (number and title) until on-device OCR
  exists ([#28](https://github.com/eRgo35/notae/issues/28)).
- The Slint UI and its `cargo-apk2` build, the vendored Slint backend patch,
  and `scripts/release.sh`, `scripts/build-apk.sh` and
  `scripts/prepare-release-keystore.sh`.
- The only `unsafe` code: every crate now forbids it.

## [0.4.0] - 2026-10-31

Maintenance release: tooling and documentation only, no app changes. The APK
is functionally v0.3.0. See
[`docs/release-notes/v0.4.0.md`](docs/release-notes/v0.4.0.md).

### Added

- `ROADMAP.md` and `GLOSSARY.md`, and a "Development workflow" section in
  `CONTRIBUTING.md`.

### Changed

- Quality gates and releases run on the maintainer's PC: `scripts/check.sh`
  (`fast` or `full`) runs the gates, an opt-in pre-push hook runs the fast
  ones, and `scripts/release.sh vX.Y.Z` builds, signs, verifies and publishes
  a release from its tag. The APK build helpers moved from `.github/scripts/`
  to `scripts/`.
- GitHub CI runs one cached job (fmt, Clippy, tests) and skips changes that
  touch only Markdown or `docs/`. The weekly audit uses the prebuilt
  `cargo-deny` binary.

### Removed

- The GitHub release workflow (`release.yml`), which held the signing key in
  repository secrets, and the Android arm64 compile job in CI (it runs in
  `scripts/check.sh full` and in every release).
- The unused `apps/notae-android/.cargo/config.toml` with machine-specific NDK
  linker paths; `cargo-apk2` configures the linker itself.
- `docs/superpowers/IMPLEMENTATION-STATE.md`, replaced by `ROADMAP.md` and the
  milestones.

### Security

- The repository is public. Its history was rewritten on 2026-10-31 to remove
  local paths, a device serial number, a personal screenshot and committed
  agent reports, and every commit was re-signed; all commit hashes changed and
  the release tags moved to the rewritten commits. Secret scanning with push
  protection is enabled.

## [0.3.0] - 2026-10-30

Polish UI, Favorites and Sets for preparing the music for each Mass. Verified
with host tests and on the Android tablet emulator; **not yet run on the
Galaxy Tab S10 FE+**. See
[`docs/release-notes/v0.3.0.md`](docs/release-notes/v0.3.0.md).

### Added

- Polish UI: every screen, dialog and status message is translated into Polish.
- Language in Settings: **System default**, **English** or **Polski**. The
  choice also follows Android's per-app language setting.
- Favorites: **Add to favorites** in a piece's ⋮ sheet stars it, and the new
  **Favorites** section lists starred pieces, newest first.
- Sets, such as one for each Mass or service: create, edit (name, optional
  date and notes) and delete a set from the new **Sets** section. Deleting asks
  for confirmation.
- Inside a set, pieces show in the set's own order with their position
  numbers. Add pieces from **Add**, or with **Add to set…** in a piece's ⋮
  sheet, and remove them with **Remove from set**. The same piece may appear
  more than once, and each copy keeps its own place.
- A piece that is no longer in the library keeps its place in a set, shown as
  "Missing from library". When a rescan finds a replaced PDF at the same path,
  its favorite and set entries are re-linked to it.
- **Export sets** in Settings saves favorites and all sets to one JSON file
  through Android's save dialog, for moving them to the next Notae app.
  Favorites and sets are stored in the app's private storage, never in the
  library folder.

### Changed

- Dependencies: `sha2` 0.11, `pollster` 1.0, `serde_json` and the GitHub
  Actions group updated; `pulldown-cmark` 0.13.5 for RUSTSEC-2026-0414.

## [0.2.2] - 2026-10-19

Follow-up to v0.2.1: Polish `ł`/`Ł` search
([#35](https://github.com/eRgo35/notae/issues/35)), import review spacing
([#34](https://github.com/eRgo35/notae/issues/34)), a phone-friendly library
grid and a ⋮ overflow action sheet. Verified with host tests and software
renders; **not yet run on a phone or emulator**, and the issues above are
awaiting verification on the Galaxy Tab S10 FE+. See
[`docs/release-notes/v0.2.2.md`](docs/release-notes/v0.2.2.md) for the full text,
including known limitations.

### Added

- `.github/scripts/build-apk.sh [release|dev]` builds the arm64 APK locally and
  prints its path and size: `release` runs the exact command of the Release
  workflow (release key required), `dev` is a quicker debug-signed build with
  the new `apk-dev` Cargo profile. The shipped `release` profile is unchanged.
  See [`docs/android-release.md`](docs/android-release.md).

### Changed

- Library on phones: the grid shows two outlined cards per row (three only on a
  wider screen) with a thumbnail, title and number · composer, like the Google
  Drive app, instead of one tile as large as the whole screen. Windows 600 px
  wide or more, such as the tablet, keep their layout.
- Item actions: every card and list row, including Recent, ends with a visible
  48 px **More actions** (⋮) button that opens a bottom sheet with **Open
  score**, **Score details** and **Edit details**. It replaces both the Info
  icon (#26) and the long-press menu, which is removed; the button is also the
  route TalkBack users had no gesture for. Tapping a card still opens the PDF.
- Index schema version 2 ([#35](https://github.com/eRgo35/notae/issues/35)):
  the search index folds `ł`/`Ł` to `l`/`L` as it is written, and existing
  indexes are migrated in place the first time this build opens them. The
  upgrade is forward-only: older builds refuse a version 2 index. The sidecar
  format is unchanged (`SIDECAR_SCHEMA_VERSION` stays `2`).
- On-device OCR ([#28](https://github.com/eRgo35/notae/issues/28)) is accepted
  in `docs/adr/0001-on-device-ocr.md` (2026-10-19) as a manual, foreground step
  started from Settings. Documentation only: it is not implemented, and no OCR
  runs on Android.

### Fixed

- Import review ([#34](https://github.com/eRgo35/notae/issues/34)): the entries
  of the "Review desktop import" sheet are separated by a gap, and a divider
  separates the "On device" and "From desktop" values inside each entry.
- Search ([#35](https://github.com/eRgo35/notae/issues/35)): the Polish letters
  `ł` and `Ł` now match `l` and `L`, so `palka`, `Galuszka`, `czesci stale`,
  `zmartwychwstal` and `lodz` find `Pałka`, `Gałuszka`, `części stałe`,
  `zmartwychwstał` and `Łódź` in titles, composers, tags and lyrics, in every
  Search scope. Queries typed with the diacritics keep working, and titles and
  lyric snippets are still shown in their original spelling. `l` and `ł` are
  now one letter for search, so `lodz` also finds a score titled `Lodz`.

## [0.2.1] - 2026-10-18

Follow-up to v0.2.0 from the first tablet feedback (issues
[#10](https://github.com/eRgo35/notae/issues/10),
[#26](https://github.com/eRgo35/notae/issues/26),
[#27](https://github.com/eRgo35/notae/issues/27),
[#29](https://github.com/eRgo35/notae/issues/29),
[#30](https://github.com/eRgo35/notae/issues/30),
[#31](https://github.com/eRgo35/notae/issues/31),
[#32](https://github.com/eRgo35/notae/issues/32),
[#17](https://github.com/eRgo35/notae/issues/17),
[#33](https://github.com/eRgo35/notae/issues/33)). Verified with host tests and
an Android API 35 x86_64 tablet emulator; **the issues above are awaiting
verification on the Galaxy Tab S10 FE+** and stay open until the maintainer has
tested them. See [`docs/release-notes/v0.2.1.md`](docs/release-notes/v0.2.1.md)
for the full text, including known limitations.

### Added

- TalkBack support ([#17](https://github.com/eRgo35/notae/issues/17)): Notae
  now publishes Slint's accessibility tree to Android (an
  `AccessibilityNodeProvider` on the activity's content view), so TalkBack and
  `uiautomator` see labelled buttons, the Search field, Settings rows and the
  other controls, and their actions (click, focus, set text, selection,
  scroll) reach the app. Nothing is tracked while no accessibility service is
  connected, and any failure in the bridge is logged and ignored instead of
  affecting the app.
- On-device OCR for image-only PDFs: a design proposal only so far
  ([#28](https://github.com/eRgo35/notae/issues/28), `docs/adr/0001-on-device-ocr.md`).
  No OCR runs on Android.

### Changed

- Library ([#26](https://github.com/eRgo35/notae/issues/26)): tapping a card or
  row opens the PDF directly in every section and in both layouts, and the card
  Edit (pencil) icon is replaced by an **Info** icon that opens the full-width
  score preview. Metadata is edited from the preview's **Edit details**; the
  long-press menu offers **Open score** and **Score details**.
- Settings ([#29](https://github.com/eRgo35/notae/issues/29)): the Library
  maintenance Scan button is gone (the Library header owns the only Scan
  action) and the maintenance card has the same density as the other Settings
  cards.
- Motion ([#31](https://github.com/eRgo35/notae/issues/31), follow-up to
  [#10](https://github.com/eRgo35/notae/issues/10)): changing between Welcome,
  Library, Search and Settings is a fade-through with a short shared-axis
  slide, the preview and the Library cross-fade, and the sidebar slides and
  fades when it is expanded or collapsed. The leaving view stays visible while
  it fades out and cannot be tapped. Disabled when Android animations are off.
- Image-only PDFs ([#33](https://github.com/eRgo35/notae/issues/33)): interim
  behaviour until on-device OCR is decided. They stay in the library and are
  searchable by title and metadata; the "No text yet" badge and the
  "Text unavailable" copy are gone, and Settings → Library maintenance shows
  **PDFs without extractable text** as a plain count.

### Fixed

- Scan progress ([#30](https://github.com/eRgo35/notae/issues/30)): the bar no
  longer shows a pre-filled stretch before real counts exist; it is an empty
  track with a moving segment until "Reading PDF text and thumbnails: N of M"
  has a total.

- Hardware keyboard ([#32](https://github.com/eRgo35/notae/issues/32)): text
  typed on a physical keyboard now honours the active keyboard layout, so the
  Polish letters (AltGr and Shift+AltGr) can be typed. Slint 1.18.1 ignored the
  layout and modifiers. The fix is upstream Slint pull request #13112, applied
  as a vendored copy of `i-slint-backend-android-activity` 1.18.1 under
  `vendor/` (see `vendor/README.md`, which also lists the removal condition).

### Removed

- The "Needs details" section, sidebar item, chip and count
  ([#27](https://github.com/eRgo35/notae/issues/27)).
- The "Text pending" section and the "No text yet" badge
  ([#33](https://github.com/eRgo35/notae/issues/33)).

### Documentation

- README presentation layer with refreshed emulator screenshots,
  `CONTRIBUTING.md`, `SECURITY.md`, issue and pull request templates
  ([#6](https://github.com/eRgo35/notae/issues/6)).
- Release notes for v0.2.1, an implementation state update and the Library,
  Settings and motion contracts in `docs/android-debugging.md`.

### Known limitations

- Motion on the tablet is unconfirmed: the emulator shows the transitions, but
  the Galaxy Tab S10 FE+ has not been checked, including whether its animator
  scale is non-zero (the app logs `notae: motion animator_duration_scale=...
  animations_enabled=...` at startup for this).
- The "enormous" maintenance card of #29 could not be reproduced on the
  emulator; the only excess height found (the 48 px Scan icon) is removed.
- On-device OCR ([#28](https://github.com/eRgo35/notae/issues/28)) is a design
  proposal only (`docs/adr/0001-on-device-ocr.md`); nothing runs OCR on Android.
- Polish input from a hardware keyboard ([#32](https://github.com/eRgo35/notae/issues/32))
  is verified on the emulator with the Polish layout only; Samsung Keyboard,
  Gboard and the keyboard cover on the Galaxy Tab S10 FE+ are not.
- TalkBack ([#17](https://github.com/eRgo35/notae/issues/17)) is verified
  through `uiautomator` and accessibility actions on the emulator, not with
  TalkBack on the tablet. There is no TalkBack action for a long press, so the
  long-press menu is not reachable that way; its two actions are available as
  the card's default action (open) and its Info button.

## [0.2.0] - 2026-10-17

Works through the twenty tablet-feedback issues (#1 to #20) and repairs the CI
pipeline (#21). Verified with host tests and an Android API 35 x86_64 tablet
emulator; **not yet accepted on the Galaxy Tab S10 FE+**. See
[`docs/release-notes/v0.2.0.md`](docs/release-notes/v0.2.0.md) for the full
text, including known limitations.

### Added

- Grid and list library layouts, switched from the header and remembered
  (#14).
- Full-width score preview with previous/next browsing (#15).
- Collapsible sidebar, remembered separately for portrait and landscape;
  portrait starts collapsed (#20).
- One themed, draggable, auto-hiding scrollbar on Library, preview, Search,
  Settings and Welcome (#19).
- Item actions: an Edit icon on every card and row, and a long-press menu with
  Open score, Score details and Edit details (#17).
- Short 120/220 ms transitions between views, sheets and options, disabled when
  Android animations are off (#10).
- A microphone action in Search when Android provides a speech recognizer (#5).
- A Ctrl K hint inside the search field when a hardware keyboard is attached
  (#4).
- A signed arm64 release workflow that builds the APK from version tags and
  attaches it, with a `.sha256`, to the GitHub Release; a weekly dependency
  advisory audit; Dependabot configuration (#21).
- README presentation layer with emulator screenshots (#6).

### Changed

- Opening a score from Recent launches the PDF directly (#16).
- Settings redesigned: library folder, Library maintenance with indexed count
  and last scan, a more informative About, no pending-text paragraph and no
  privacy card; the Scan icon is gone from the Settings header
  (#8, #9, #11, #12).
- A scan reports the phase it is really in (Scanning library, Indexing N PDFs,
  Reading PDF text and thumbnails N of M, Checking for a desktop sidecar) and
  completion only after the sidecar check; "Text pending" wording now says what
  to do next (#18).
- Higher-contrast launcher icon with rounded legacy masks (#7).
- Compact Welcome spacing (#8).

### Fixed

- The app no longer flashes an intermediate screen before All scores on startup
  (#1).
- Polish letters (ą ć ę ł ń ó ś ź ż) type into Search and are found by
  full-text search (#13).
- The search field is focused once after the transition settles, so the first
  tap should no longer show a stray cursor handle (#2).
- Scope chips no longer shift when pressed (#3).
- CI restored: host quality gates and the Android arm64 compile (#21).

### Removed

- The Scan button in Search (#5) and the "Press Ctrl K" sentence (#4).

### Known limitations

- TalkBack cannot operate the item actions and menus: Slint's Android backend
  publishes no accessibility tree (#17).
- The signed arm64 APK is verified (version, signature, SHA-256) but has not
  been launched on an arm64 device by the release process.
- Installing over a debug-key build requires uninstalling it first, which
  deletes the app's private data; see
  [`docs/android-release.md`](docs/android-release.md).

[Unreleased]: https://github.com/eRgo35/notae/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/eRgo35/notae/releases/tag/v0.4.0
[0.3.0]: https://github.com/eRgo35/notae/releases/tag/v0.3.0
[0.2.2]: https://github.com/eRgo35/notae/releases/tag/v0.2.2
[0.2.1]: https://github.com/eRgo35/notae/releases/tag/v0.2.1
[0.2.0]: https://github.com/eRgo35/notae/releases/tag/v0.2.0
