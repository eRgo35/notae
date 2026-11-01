//! Pure domain model for Notae.
//!
//! The serde representations of these types are stable, and they depend on no
//! storage or platform crate.

mod filename;
mod identity;
mod metadata;
mod ordering;
mod status;
mod text;

pub use filename::{
    FilenameError, ParsedFilename, is_note_filename, parse_filename, parse_note_filename,
};
pub use identity::{EpochTimestamp, NoteId, PdfFingerprint, RelativePdfPath, Sha256Digest};
pub use metadata::{
    DomainError, FieldProvenance, IndexedNote, MetadataField, MetadataOrigin, NoteMetadata,
};
pub use ordering::{DuplicateId, compare_library_order, validate_unique_ids};
pub use status::IndexStatus;
pub use text::normalize_whitespace;
