# Changelog

All notable changes to Notae are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and Notae adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
The hand-written release notes for each version live in
[`docs/release-notes/`](docs/release-notes/).

## [Unreleased]

> **Work in progress for v0.2.1.** Nothing in this section is released, and
> some items are not merged to `main` yet. The milestone is
> [v0.2.1](https://github.com/eRgo35/notae/milestone/2); entries are named by
> issue number and will be reworded when the release notes are written.

### Added

- On-device OCR for image-only PDFs: a design proposal only so far, tracked in
  [#28](https://github.com/eRgo35/notae/issues/28). No OCR runs on Android yet.

### Changed

- Library ([#26](https://github.com/eRgo35/notae/issues/26)): open the PDF
  directly from every section, and replace the card Edit icon with an Info icon
  that opens the score details.
- Settings ([#29](https://github.com/eRgo35/notae/issues/29)): remove the
  Library maintenance Scan button and fix the oversized maintenance card.
- Sidebar ([#31](https://github.com/eRgo35/notae/issues/31)): add a transition
  when the sidebar is expanded or collapsed. This follows up
  [#10](https://github.com/eRgo35/notae/issues/10), whose view-change
  transitions were not visible on the tablet.

### Fixed

- Scan progress ([#30](https://github.com/eRgo35/notae/issues/30)): the bar no
  longer jumps to 33% as soon as a scan starts. Follow-up to the phase-accurate
  scan status of [#18](https://github.com/eRgo35/notae/issues/18).
- Polish letters cannot be typed with a physical keyboard
  ([#32](https://github.com/eRgo35/notae/issues/32)).

### Removed

- The "Needs details" section ([#27](https://github.com/eRgo35/notae/issues/27)).
- The "Text pending" section and the "No text yet" badge
  ([#33](https://github.com/eRgo35/notae/issues/33)).

### Documentation

- README presentation layer, `CONTRIBUTING.md`, `SECURITY.md`, issue and pull
  request templates ([#6](https://github.com/eRgo35/notae/issues/6)).

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

[Unreleased]: https://github.com/eRgo35/notae/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/eRgo35/notae/releases/tag/v0.2.0
