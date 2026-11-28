# Roadmap

Notae has one maintainer. Work is tracked in
[GitHub issues](https://github.com/eRgo35/notae/issues) and
[milestones](https://github.com/eRgo35/notae/milestones); this page is the
short version.

## Released: 0.11.0

0.5.0 replaced the Rust UI with a Jetpack Compose app over a UniFFI Rust core
(application id `com.c2yz.notae`, Android only). 0.6.0 added phone layouts,
0.6.1 added motion, and 0.7.0 slides between screens, shows sets as a stack of
score previews, adds score notes and set notes, and shares a set. 0.7.1 adds the
privacy policy and the open-source licenses and prepares Notae for Google Play.
0.8.0 adds Notae's own PDF viewer and drawings on the scores of a set
([ADR 0003](docs/adr/0003-in-app-viewer-and-overlays.md)) and near-instant
scans with the PC sidecar ([ADR 0004](docs/adr/0004-pc-sidecar-and-automatic-scans.md)).
0.9.0 adds the viewer's layouts and scales, zoom anywhere, the notes panel on
a tablet, and the library from a ZIP file or a link; 0.9.1 fixes small
things in that viewer. 0.9.2 follows the schola's new folder layout and
fixes bugs found in a review, and 0.10.0 fixes the rest of them and moves to
compileSdk 37 and UniFFI 0.33. 0.11.0 plays a set from piece to piece, keeps
the tablet's navigation in one place and fixes the search order. See
[`CHANGELOG.md`](CHANGELOG.md).

Notae has been on Google Play (internal testing) since 0.7.1
([`docs/android-release.md`](docs/android-release.md)).

## Later

- On-device OCR for scanned PDFs, started manually and run in the foreground
  ([#28](https://github.com/eRgo35/notae/issues/28),
  [ADR 0001](docs/adr/0001-on-device-ocr.md)).
- Turning scanned scores into music notation shown in Notae, with comments
  pinned to the music (research,
  [#52](https://github.com/eRgo35/notae/issues/52)).

## Not planned

- Cloud sync, accounts or any network access.
- Desktop or iOS apps.
