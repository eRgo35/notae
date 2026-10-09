//! Pure domain model for Notae.
//!
//! These types cross the portable sidecar boundary, so their serde
//! representations are stable and they depend on no storage or platform crate.

mod identity;
mod metadata;
mod status;

pub use identity::{EpochTimestamp, NoteId, PdfFingerprint, RelativePdfPath, Sha256Digest};
pub use metadata::{
    DomainError, FieldProvenance, MetadataField, MetadataOrigin, NoteMetadata,
};
pub use status::IndexStatus;