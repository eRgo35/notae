# 0004. PC sidecar and automatic scans

- **Status:** Accepted (2026-11-22; maintainer decisions for 0.8.0)
- **Issues:** #55 (preloaded text)
- **Supersedes:** the sidecar removal in ADR 0002 item 8, in part (the sidecar returns, in a new form), and the rule in ADR 0001 that all work is "started by the user", which now applies to OCR only.
- **Design:** [`docs/superpowers/specs/2026-11-22-fast-scan-and-pc-sidecar-design.md`](../superpowers/specs/2026-11-22-fast-scan-and-pc-sidecar-design.md)

## Context

Every scan reads every PDF end to end to hash it and extracts the text of every page again, even when nothing changed. Importing the schola library takes minutes, and picking the same folder again feels like a rebuild. The maintainer can prepare the library once on the PC (hashes, metadata, text, thumbnails), and the tablet can take those results instead of recomputing them.

## Decision

1. **The PC sidecar returns** as `.notae/library.json` plus `.notae/thumbnails/`, written by the `notae-sidecar` PC tool and committed with the scores. Notae reads it and never writes it, so the library stays read-only. The 0.3.0 sidecar (`index.sqlite`, `manifest.json`) stays removed and is ignored.
2. **Trust path and size.** A PDF whose library-relative path and byte size both equal a sidecar entry takes the entry's SHA-256 without being read. The modified time is not part of the match, because copies and clones reset it. A mismatch falls back to reading the file.
3. **Precedence of metadata:** `Manual` > `PcImported` > `FilenameDerived`. A `PcImported` value replaces a `FilenameDerived` one, and a field the user edited keeps the user's value.
4. **Sidecar text wins.** It replaces any text already in the index for that file and the note becomes `Ready`. An entry with no text falls back to the PDF's own text layer.
5. **Automatic rescans** when a folder is picked and when the app starts with a saved library that opens. It is the same operation as tapping Rescan, with the same progress banner and cancel. "Started by the user" in ADR 0001 now applies to OCR only.
6. **No network access is added.**

## Options considered

- **`.notae/library.json` plus `.notae/thumbnails/<sha256>.png`**: committed with the scores, so copies and clones carry it; readable git diffs; `serde_json` is already a dependency; decoupled from the app's index schema. Chosen.
- **`.notae/index.sqlite`, as in 0.3.0**: binary in git, and it ties the PC tool to the app's schema, which made the 0.3.0 sidecar brittle. Rejected.
- **The tablet reads `Indeks.csv`; the PC adds only hashes**: couples the app to Polish column names, splits data across two files, and still needs the PC for hashes and thumbnails. Rejected.

## Consequences

- The index records the size and modified time it last saw for each path (schema version bump with a migration), so a rescan of an unchanged library reads no PDF.
- A sidecar that does not parse, or has an unknown `version`, is ignored with a warning in the scan summary.
- A wrong sidecar entry would attach wrong data to a file, so the PC tool writes nothing when it hits an error, and the tablet ignores entries whose path or size differ.
- The comment in `state.rs` ("No automatic rescan") and the host smoke-test contract change.
- The glossary entries for **Scan** and **Sidecar** change.
- No network access is added.
