//! SQLite persistence and full-text search for the Notae library.
//!
//! [`IndexStore`] is the only entry point: it owns one bundled SQLite
//! connection with FTS5 enabled, applies the schema migrations in
//! [`schema`], and stores the canonical [`IndexedNote`](notae_domain::IndexedNote)
//! metadata in ordinary columns next to a derived FTS5 search index.
//!
//! # Layout
//!
//! - `notes` — one row per indexed PDF: path, SHA-256, size and modification
//!   metadata, note ID, title, composer/artist, tags, per-field provenance,
//!   pipeline status, and normalized OCR text. The note ID is unique when
//!   present, which is what makes the ID invariant a database constraint
//!   rather than a convention.
//! - `notes_fts` — an FTS5 index over note ID, title, composer/artist, tags,
//!   filename, and OCR text, sharing row identifiers with `notes`.
//! - `schema_migrations` — the versions applied to this database.
//!
//! # OCR text
//!
//! OCR text lives in its own [`OcrText`] value beside the note, written through
//! [`IndexStore::upsert_note_with_ocr`] or replaced later with
//! [`IndexStore::set_ocr`]. See [`OcrText`] for why it is not part of
//! `IndexedNote`.

mod schema;
mod search;
mod store;

pub use crate::schema::{MIGRATIONS, Migration, SCHEMA_VERSION};
pub use crate::search::SearchHit;
pub use crate::store::{IndexError, IndexStore, OcrText};
