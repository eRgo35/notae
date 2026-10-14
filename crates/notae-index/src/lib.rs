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
//! - `schema_migrations` — the versions applied to this database. A recorded
//!   version is verified against the objects it names when the database is
//!   opened, and the table itself is created inside the migration transaction,
//!   so a failed migration leaves no trace.
//!
//! # OCR text
//!
//! OCR text lives in its own [`OcrText`] value beside the note, written through
//! [`IndexStore::reindex_note_with_ocr`] or replaced later with
//! [`IndexStore::set_ocr`]. See [`OcrText`] for why it is not part of
//! `IndexedNote`.
//!
//! # Writing records
//!
//! [`IndexStore::reindex_note`] is the write a rescan uses. It resolves the two
//! unique keys a record has — content hash and path — and reports which of the
//! four resulting situations it wrote, as a [`ReindexOutcome`], so a caller
//! never has to compare rows to tell a moved file from a changed one.
//!
//! # Reconciliation
//!
//! [`Reconciler`] compares the stored records against the PDFs a
//! [`PdfCatalog`] enumerates. [`Reconciler::plan`] is read-only and returns a
//! [`RescanPlan`] the caller can preview and abandon; [`Reconciler::apply`]
//! performs it and returns a [`RescanReport`]. File identity is the SHA-256 of
//! the contents, so a rename moves one record instead of creating a second, and
//! manually edited metadata in [`crate::provenance`] is never overwritten by a
//! filename-derived value.
//!
//! A catalog reports what it could not use through a [`FileCondition`], so a
//! file that is unreadable or is not a PDF becomes a [`FileFailure`] with an
//! actionable reason and keeps any record it already had — one bad PDF never
//! aborts a rescan and never deletes a good record. A note ID claimed by two
//! files is reported as a [`DuplicateIdConflict`]: both files stay in the
//! library, both are marked [`notae_domain::IndexStatus::DuplicateIdConflict`],
//! and only one of them holds the ID until the user resolves it.
//!
//! # The PC sidecar
//!
//! [`SidecarWriter`] turns an index into a portable, versioned manifest that the
//! Linux CLI writes beside its SQLite payload, and [`SidecarImporter`] brings
//! one into an existing library. [`SidecarImporter::preview`] is read-only and
//! classifies every entry as new, changed, unchanged, absent, unreadable, or a
//! hash mismatch — against the files in the library root, read through a
//! [`PdfCatalog`], not against what the index last fingerprinted — and reports
//! missing records, OCR availability, duplicate-ID conflicts, and the metadata
//! that is preserved locally versus proposed by the sidecar.
//! [`SidecarImporter::apply`] performs only what the user confirmed.
//! [`SidecarManifest::validate`] runs before any mutation, so a malformed or
//! unsupported sidecar leaves the library untouched.

mod import;
mod provenance;
mod reconcile;
mod schema;
mod search;
mod sidecar;
mod store;

pub use crate::import::{
    EntryChange, ImportConflict, ImportEntry, ImportFailure, ImportPreview, ImportReport,
    MetadataImport, OcrAssessment, SidecarImporter,
};
pub use crate::provenance::{
    MetadataRefreshError, RefreshOutcome, metadata_from_filename, refresh_from_filename,
};
pub use crate::reconcile::{
    CatalogError, DuplicateIdConflict, FileCondition, FileFailure, FileFailureReason, PdfCatalog,
    PdfFile, PlannedAddition, PlannedDeletion, PlannedMove, PlannedUpdate, ReconcileError,
    Reconciler, RescanPlan, RescanReport, UnchangedFile,
};
pub use crate::schema::{MIGRATIONS, Migration, SCHEMA_VERSION};
pub use crate::search::{SearchHit, SearchScope};
pub use crate::sidecar::{
    SIDECAR_SCHEMA_VERSION, SidecarEntry, SidecarError, SidecarManifest, SidecarOcr, SidecarWriter,
};
pub use crate::store::{IndexError, IndexStore, OcrText, ReindexOutcome};
