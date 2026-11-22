# Fast scan and the PC sidecar (0.8.0)

- **Date:** 2026-11-22
- **Milestone:** v0.8.0 (with the layout audit, the in-app viewer #46 and drawings #48)
- **Related:** #55 (preloaded text), ADR 0001 (OCR, no automatic work), ADR 0002 item 8 (sidecar removed)

## Intent

Importing the schola library should take seconds, not minutes, and picking the same folder again
should cost nothing. The maintainer prepares the library once on the PC (hashes, metadata, text,
thumbnails) and the tablet takes those results instead of recomputing them.

Today every scan reads every PDF end to end to hash it (`SafLibrary.kt` `readEntry`) and extracts the
text of every page of every PDF again (`rescan.rs` `apply_pdf_text_layer`), even when nothing changed.
The index survives a folder pick, but the next scan redoes all of that work, so it feels like a rebuild.

## Goals

1. **Unchanged files cost nothing.** A rescan reads no PDF whose path, size and last-modified time match
   what the index recorded. Re-picking the same folder is a listing and nothing more.
2. **The PC sidecar.** A tool on the PC writes `.notae/` into the schola folder. On the tablet, a PDF whose
   path and size match a sidecar entry takes its SHA-256, number, title, composer, tags, text and
   thumbnail from the sidecar without opening the PDF.
3. **Automatic scans.** Notae scans when a folder is picked and when the app starts with a library open.
   **Rescan** stays for the user.

## Non-goals

- OCR. The schola PDFs already carry a Polish and English text layer, and `Indeks.csv` already holds the
  full OCR text. The tool runs no Tesseract. (On-device OCR stays ADR 0001 / #28.)
- The background OCR check from #55.
- Downloading the library (#54, 0.10.0). The sidecar is designed so a clone brings it along.
- Writing to the library folder from the tablet. The library stays read-only.

## Anti-goals

- A sidecar that silently attaches the wrong data to a file. Every match needs the same path **and** size,
  and a mismatch falls back to reading the file.
- Overwriting what the user edited on the tablet.
- Making the tablet depend on `Indeks.csv` or its Polish column names. Only the PC tool reads the CSV.

## Decisions (maintainer)

- Scan automatically on folder pick and app start.
- The sidecar carries hashes, metadata, text **and** thumbnails (A + B + C).
- **Trust path and size.** The tablet trusts the sidecar's SHA-256 when path and byte size match. Copies
  and clones reset modified times, so the time is not part of the sidecar match.
- Metadata comes from `Indeks.csv` in the schola repo: `ID` → number, `Tytuł` → title, `Autor` →
  composer, `Tekst OCR` → text.
- **Tags:** every `Rodzaj` value becomes a tag, plus the values of a new `Tagi` column, separated by `;`.
  The maintainer adds the column.
- Sidecar format: option 1 below. The PC tool may use the `csv` crate.

## Approaches considered

| Option | Verdict | Why |
| --- | --- | --- |
| **`.notae/library.json` + `.notae/thumbnails/<sha256>.png`** | **Chosen** | Committed with the scores, so copies and clones carry it; readable git diffs; `serde_json` is already a dependency; decoupled from the app's index schema. |
| `.notae/index.sqlite`, as in 0.3.0 | Rejected | Binary in git; ties the PC tool to the app's schema, which made 0.3.0's sidecar brittle. |
| The tablet reads `Indeks.csv`; PC adds only hashes | Rejected | Couples the app to Polish column names, splits data across two files, still needs the PC for hashes and thumbnails. |

PC tool: calling `pdftoppm` for thumbnails was chosen over linking a PDF renderer crate, so the tool needs
no crate beyond `csv`.

## Design

### 1. Unchanged files are skipped

- `PlatformLibrary.enumerate` stops hashing. `DocumentEntry` gains `modified_millis: Option<i64>` (from
  `DocumentsContract.Document.COLUMN_LAST_MODIFIED`), and `content` is no longer filled in by listing.
- A new callback `hash(handle, path) -> DocumentContent` reads and hashes one document, as `readEntry` does
  today.
- The index records, per path, the size and modified time it last saw (schema version 3, with a migration
  from 2). A listed file whose path, size and time match is reused as it is: no hash, no text extraction,
  no thumbnail render (the thumbnail cache already holds it).
- Everything else is resolved in order: sidecar match (below), otherwise `hash`, then the existing text
  layer and thumbnail steps for that file only.
- A missing modified time (`None`) never matches, so that file is hashed.
- Reconcile (moves, duplicates, deletions) keeps working on SHA-256s as it does now.

### 2. The sidecar on the tablet

- At the start of a scan the core reads `.notae/library.json` from the library root through a new callback
  `read_file(handle, path) -> Option<Vec<u8>>` (`None` when it does not exist). A sidecar that does not
  parse, or has an unknown `version`, is ignored with a warning in the scan summary; the scan goes on
  without it.
- `enumerate` skips everything under `.notae/`.
- A listed PDF whose path and size equal a sidecar entry takes:
  - **SHA-256** as its content key, without being read.
  - **Number, title, composer, tags** with origin `PcImported` (already in `MetadataOrigin`). Any field with
    origin `Manual` keeps the user's value. A `PcImported` value replaces a `FilenameDerived` one.
  - **Text** as the note's searchable text. The PDF's own text layer is not extracted for that file.
  - **Thumbnail:** `.notae/thumbnails/<sha256>.png` is copied into the thumbnail cache through
    `read_file` when the cache lacks it. A missing PNG falls back to rendering on the device.
- Sidecar entries for files not on disk are ignored.
- **A changed sidecar re-applies.** *(Builder's call, recorded.)* The index stores, per path, a hash of the
  sidecar entry it last applied. On every scan the sidecar is parsed (cheap); an entry whose hash differs
  from the stored one is re-applied to its file even when the file itself is unchanged, so re-running the
  tool with a new `Tagi` column reaches the tablet. `Manual` fields still win. Unchanged entries cost nothing.
- **Sidecar text wins.** *(Builder's call.)* Sidecar text replaces any text already in the index for that
  file (including text extracted from the PDF before 0.8.0), and the note becomes `Ready`. An entry with
  `null` or empty `text` falls back to extracting the PDF's text layer as today.

### 3. Automatic scans

- The core starts a rescan after a folder is opened (`open_root` succeeds) and when the app starts with a
  saved library that opens. The rescan is the same operation as tapping **Rescan**: same progress banner,
  same cancel.
- The comment in `state.rs` ("No automatic rescan") and the host smoke-test contract change accordingly.
- A new **ADR 0004** records: the PC sidecar returns in this form (partly superseding ADR 0002 item 8), the
  trust rule, and automatic scans on open (ADR 0001's "started by the user" rule stays for OCR only).

### 4. `library.json`

```json
{
  "version": 1,
  "generator": "notae-sidecar 0.8.0",
  "scores": [
    {
      "path": "001 - Adeste Fideles.pdf",
      "size": 201714,
      "sha256": "…64 lowercase hex…",
      "number": 1,
      "title": "Adeste Fideles",
      "composer": null,
      "tags": ["Z katalogu"],
      "text": "Adeste Fideles. 1. Adeste fideles laeti triumphantes, …"
    }
  ]
}
```

`path` is library-relative and `/`-separated, as `DocumentEntry.path`. Absent or empty CSV fields become
`null` (or `[]` for tags). Entries are sorted by path, so re-running the tool on unchanged input produces
an identical file.

### 5. The PC tool: `apps/notae-sidecar`

- A new workspace member, a binary: `notae-sidecar <library-folder>`. Dependencies: `notae-domain`,
  `serde`, `serde_json`, `sha2` (all already in the workspace) and `csv` (approved).
- It walks the folder for `*.pdf` (skipping `.notae/` and the repo's `sources/`), hashes each file, parses
  number and title from the file name with Notae's own parser, then overlays the `Indeks.csv` row matched
  by `Nazwa pliku`: `ID`, `Tytuł`, `Autor`, `Rodzaj` + `Tagi`, `Tekst OCR`. The CSV is UTF-8 with a BOM.
- It renders thumbnails with `pdftoppm -png -f 1 -l 1 -scale-to-x 400 -scale-to-y -1`, matching the
  device's 400 px width (`THUMBNAIL_WIDTH_PX`), and writes only missing ones. It deletes thumbnails whose
  hash no longer appears.
- It writes `library.json` atomically (temp file, then rename).
- **Errors write nothing.** *(Builder's call.)* CSV rows with no PDF, an unreadable CSV, or `pdftoppm`
  missing or failing are errors: the tool reports all of them, exits non-zero and leaves `.notae/`
  untouched, so a partial sidecar is never committed. PDFs with no CSV row are warnings (filename metadata
  only) and the file is still written.
- It leaves files it does not own alone. The schola repo still tracks the 0.3.0 sidecar
  (`.notae/index.sqlite`, `.notae/manifest.json`); the maintainer removes them with `git rm` once, and the
  0.8.0 tablet ignores them.
- The thumbnail renderer sits behind a function the tests replace, so the tool's tests need no `pdftoppm`
  on CI.
- The maintainer runs it in the schola repo and commits `.notae/`.
- Current data (checked 2026-11-22): all 176 CSV rows match a PDF and every PDF has a row; two rows with an
  empty `ID` get `number: null`.

### Testing

- Rust unit tests: sidecar parsing and version handling, the path+size match, `Manual` beats
  `PcImported` beats `FilenameDerived`, unchanged files skipped, schema migration 2 → 3.
- Operation tests: a rescan over an unchanged library calls `hash`, `extract_text_layer` and
  `render_thumbnail` zero times; a sidecar-matched file calls none of them.
- PC tool: tests over a fixture folder with a small CSV (quoted, multi-line text, BOM).
- Kotlin: `SafLibraryTest` for listing with modified time and the new `hash` and `read_file` callbacks.
- On the tablet: import the schola folder with `.notae/`, time it, and record it in the RC checklist.

## Left to the builder

- Names of the new callbacks, tables and columns; how the per-path state is stored in the index.
- How scan progress counts files that were skipped.
- Whether the thumbnail copy goes through `read_file` or a dedicated copy callback, if large reads over
  UniFFI turn out slow.
- CLI output format of `notae-sidecar`.
