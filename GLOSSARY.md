# Glossary

The domain terms Notae's code, docs and issues use. Use these words; avoid the synonyms listed.

- **Library**: the one folder of PDFs the user picks (a Storage Access Framework grant on Android, the path argument of `notae-index`). Notae reads it and never modifies the PDFs in it. _Avoid:_ collection, catalogue (for the folder).
- **Score / piece**: one PDF in the library and its one record in the index. The UI says "score"; the favorites and sets code says "piece". _Avoid:_ note, song, file (except for the PDF itself).
- **Number** and **title**: parsed from a file named `<number> - <title>.pdf`. Other names work and have no number.
- **Details**: the editable metadata of a score (number, title, composer, tags). Edits live in the index and never rename the PDF.
- **File identity**: the SHA-256 content hash that keeps a renamed or moved PDF one record.
- **Scan**: walking the library to build or refresh the index. Always started by the user, never in the background. _Avoid:_ sync.
- **Index**: the SQLite database (with FTS5 search) built from a library. On Android it is private to the app.
- **Sidecar**: the `.notae/` directory (`index.sqlite` and `manifest.json`) that `notae-index` writes next to the PDFs and the app imports after an **import preview**. Removed in 0.5.0.
- **Text layer**: text embedded in a PDF, read on-device on Android 15 and later.
- **Lyrics**: the searchable page text of a score, from its text layer or from OCR. The code calls it page text (`SearchScope::Text`). _Avoid:_ content.
- **Scope**: which fields a search looks at: All, Titles, Composers, Lyrics or Tags (`SearchScope`).
- **Favorite**: a starred score, listed newest first in Favorites.
- **Set**: a named, ordered list of references to scores, for example one per Mass, with an optional date and notes. A score can be in several sets. _Avoid:_ playlist, setlist.
- **Recent**: the scores opened most recently.
