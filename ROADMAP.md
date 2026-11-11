# Roadmap

Notae has one maintainer. Work is tracked in
[GitHub issues](https://github.com/eRgo35/notae/issues) and
[milestones](https://github.com/eRgo35/notae/milestones); this page is the
short version.

## Released: 0.7.0

0.5.0 replaced the Rust UI with a Jetpack Compose app over a UniFFI Rust core
(application id `com.c2yz.notae`, Android only). 0.6.0 added phone layouts,
0.6.1 added motion, and 0.7.0 slides between screens, shows sets as a stack of
score previews, adds score notes and set notes, and shares a set. See
[`CHANGELOG.md`](CHANGELOG.md).

## Next: 0.8.0

[Milestone v0.8.0](https://github.com/eRgo35/notae/milestone/7). Notae's own
PDF viewer, the new default (the system's default app stays an option in
Settings), and drawings on a score within a set, kept as an overlay that never
changes the PDF ([ADR 0003](docs/adr/0003-in-app-viewer-and-overlays.md)).

## Later

- Google Play release
  ([#40](https://github.com/eRgo35/notae/issues/40)).
- On-device OCR for scanned PDFs, started manually and run in the foreground
  ([#28](https://github.com/eRgo35/notae/issues/28),
  [ADR 0001](docs/adr/0001-on-device-ocr.md)).
- Turning scanned scores into music notation shown in Notae, with comments
  pinned to the music (research,
  [#52](https://github.com/eRgo35/notae/issues/52)).

## Not planned

- Cloud sync, accounts or any network access.
- Desktop or iOS apps.
