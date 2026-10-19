# Changelog

All notable changes to Notae are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and Notae adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
The hand-written release notes for each version live in
[`docs/release-notes/`](docs/release-notes/).

## [Unreleased]

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

### Changed

- Index schema version 2 ([#35](https://github.com/eRgo35/notae/issues/35)):
  the search index folds `ł`/`Ł` to `l`/`L` as it is written, and existing
  indexes are migrated in place the first time this build opens them. The
  upgrade is forward-only: older builds refuse a version 2 index. The sidecar
  format is unchanged (`SIDECAR_SCHEMA_VERSION` stays `2`).

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

[Unreleased]: https://github.com/eRgo35/notae/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/eRgo35/notae/releases/tag/v0.2.1
[0.2.0]: https://github.com/eRgo35/notae/releases/tag/v0.2.0
