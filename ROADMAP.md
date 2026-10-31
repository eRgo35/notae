# Roadmap

Notae has one maintainer. Work is tracked in
[GitHub issues](https://github.com/eRgo35/notae/issues) and
[milestones](https://github.com/eRgo35/notae/milestones); this page is the
short version.

## Now: 0.4.0

[Milestone v0.4.0](https://github.com/eRgo35/notae/milestone/4). Maintenance
only, with no app changes: quality gates and releases run on the maintainer's
PC (`scripts/check.sh`, `scripts/release.sh`), GitHub CI runs a light check,
and the README, ROADMAP, GLOSSARY and CONTRIBUTING describe the project as it
is. See [`CONTRIBUTING.md`](CONTRIBUTING.md#development-workflow).

## Next: 0.5.0

[Milestone v0.5.0](https://github.com/eRgo35/notae/milestone/5). The Slint UI
is replaced by a Jetpack Compose app over a UniFFI Rust core, with the new
application id `com.c2yz.notae`. Notae becomes Android only: the Linux indexer
(`notae-index`), sidecar import and `notae-ocr` are removed.

## Later

- Google Play release, after 0.5.0
  ([#40](https://github.com/eRgo35/notae/issues/40)).
- On-device OCR for scanned PDFs, started manually and run in the foreground
  ([#28](https://github.com/eRgo35/notae/issues/28),
  [ADR 0001](docs/adr/0001-on-device-ocr.md)).

## Not planned

- Cloud sync, accounts or any network access.
- Viewing or annotating PDFs inside Notae: scores open in the PDF viewer you
  already use.
- Desktop or iOS apps.
