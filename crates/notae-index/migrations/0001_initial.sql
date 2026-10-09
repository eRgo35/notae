-- Notae index schema, version 1.
--
-- Canonical metadata lives in ordinary STRICT columns so ID uniqueness,
-- library ordering, and field editing never depend on tokenization. The FTS5
-- table is a derived search index whose rowid equals `notes.row_id`; it is
-- rewritten from the note row on every write and is never read as a source of
-- truth.

CREATE TABLE notes (
    row_id              INTEGER PRIMARY KEY,
    -- Root-relative path; the human-facing identity of a file on disk.
    path                TEXT NOT NULL UNIQUE,
    -- Lowercase hex SHA-256; file identity for rename and change detection.
    sha256              TEXT NOT NULL UNIQUE,
    byte_len            INTEGER NOT NULL,
    modified_secs       INTEGER,
    modified_nanos      INTEGER,
    -- Optional, unique when present. SQLite allows many NULLs in a UNIQUE
    -- column, so unnumbered records never conflict.
    note_id             INTEGER,
    title               TEXT NOT NULL,
    composer_or_artist  TEXT,
    -- Normalized tags joined with newlines; normalization removes whitespace
    -- runs, so a newline cannot appear inside a tag.
    tags                TEXT NOT NULL,
    provenance_id       TEXT NOT NULL,
    provenance_title    TEXT NOT NULL,
    provenance_composer TEXT NOT NULL,
    provenance_tags     TEXT NOT NULL,
    status              TEXT NOT NULL,
    -- Normalized OCR text: whitespace runs collapsed to single spaces.
    ocr_text            TEXT NOT NULL DEFAULT '',
    ocr_language        TEXT,
    indexed_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE UNIQUE INDEX notes_note_id_unique
    ON notes (note_id)
    WHERE note_id IS NOT NULL;

CREATE INDEX notes_status_idx ON notes (status);

CREATE VIRTUAL TABLE notes_fts USING fts5 (
    note_id,
    title,
    composer,
    tags,
    filename,
    ocr_text,
    tokenize = 'unicode61 remove_diacritics 2'
);