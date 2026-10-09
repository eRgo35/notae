//! Pure domain model for Notae.
//!
//! These types cross the portable sidecar boundary, so their serde
//! representations are stable and they depend on no storage or platform crate.

mod filename;
mod identity;
mod metadata;
mod ordering;
mod status;

pub use filename::{ParsedFilename, parse_filename};
pub use identity::{EpochTimestamp, NoteId, PdfFingerprint, RelativePdfPath, Sha256Digest};
pub use metadata::{
    DomainError, FieldProvenance, IndexedNote, MetadataField, MetadataOrigin, NoteMetadata,
};
pub use ordering::{DuplicateId, compare_library_order, validate_unique_ids};
pub use status::IndexStatus;