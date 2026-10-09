//! The default library browsing order and the unique-ID invariant.

use std::cmp::Ordering;
use std::collections::HashMap;

use thiserror::Error;

use crate::{IndexedNote, MetadataField, NoteId, PdfFingerprint, RelativePdfPath};

/// Two records claim the same numeric ID.
///
/// A duplicate is an import conflict that the user must resolve; reporting it
/// rather than silently renumbering or hiding either record is the invariant.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("note id {id} is used by both {existing} and {conflicting}")]
pub struct DuplicateId {
    /// The contested ID.
    id: NoteId,
    /// The first record seen with this ID.
    existing: RelativePdfPath,
    /// The later record claiming the same ID.
    conflicting: RelativePdfPath,
}

impl DuplicateId {
    /// Returns the contested ID.
    #[must_use]
    pub const fn id(&self) -> NoteId {
        self.id
    }

    /// Returns the first record holding this ID.
    #[must_use]
    pub const fn existing_path(&self) -> &RelativePdfPath {
        &self.existing
    }

    /// Returns the later record claiming the same ID.
    #[must_use]
    pub const fn conflicting_path(&self) -> &RelativePdfPath {
        &self.conflicting
    }
}

/// Orders records the way the library displays them.
///
/// # Contract
///
/// This is a **total order over every distinct [`IndexedNote`] value**: for any
/// two records `a` and `b`, exactly one of `a < b`, `a == b`, or `a > b` holds,
/// and `a == b` only when `a` and `b` are the same value. There is no
/// precondition on the input, no panic, and no record is dropped, so
/// `sort_by(compare_library_order)` is deterministic for any library — including
/// one holding records that share a path, an ID, a title, or any combination of
/// them.
///
/// The comparison is a lexicographic chain over every field of the record, in
/// this order:
///
/// 1. the note ID, with numbered records before unnumbered ones and ascending
///    numeric order among numbered ones, so `2` precedes `10`;
/// 2. the case-folded title, so `Apple` and `apricot` interleave as the user
///    reads them;
/// 3. the exact title;
/// 4. the library-relative path;
/// 5. the composer or artist;
/// 6. the tag set;
/// 7. the per-field provenance, in [`MetadataField::ALL`] order;
/// 8. the fingerprint, by SHA-256, then byte length, then modification time;
/// 9. the [`crate::IndexStatus`].
///
/// The first three keys are what the user perceives as the browsing order. The
/// remaining keys exist purely to make the order total: they can never be
/// reached by two records that differ in a field the user sees first, but they
/// do decide between records that are otherwise identical, such as two records
/// at the same path whose content was re-fingerprinted or whose pipeline status
/// advanced.
///
/// Duplicate IDs are surfaced by [`validate_unique_ids`] rather than
/// renumbered, so records sharing an ID coexist in the library and must still
/// sort deterministically; the chain above guarantees they do.
#[must_use]
pub fn compare_library_order(left: &IndexedNote, right: &IndexedNote) -> Ordering {
    match (left.id(), right.id()) {
        (Some(left_id), Some(right_id)) => left_id.cmp(&right_id),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
    .then_with(|| case_folded(left.title()).cmp(&case_folded(right.title())))
    .then_with(|| left.title().cmp(right.title()))
    .then_with(|| left.path().cmp(right.path()))
    .then_with(|| {
        left.metadata()
            .composer_or_artist()
            .cmp(&right.metadata().composer_or_artist())
    })
    .then_with(|| left.metadata().tags().cmp(right.metadata().tags()))
    .then_with(|| compare_provenance(left, right))
    .then_with(|| compare_fingerprints(left.fingerprint(), right.fingerprint()))
    .then_with(|| left.status().cmp(&right.status()))
}

/// Compares the per-field provenance of two records in [`MetadataField::ALL`]
/// order.
fn compare_provenance(left: &IndexedNote, right: &IndexedNote) -> Ordering {
    MetadataField::ALL
        .into_iter()
        .map(|field| {
            (
                left.metadata().origin_of(field),
                right.metadata().origin_of(field),
            )
        })
        .find(|(left, right)| left != right)
        .map_or(Ordering::Equal, |(left, right)| left.cmp(&right))
}

/// Compares two fingerprints field by field, so records at the same path are
/// still ordered once their contents or timestamps differ.
fn compare_fingerprints(left: &PdfFingerprint, right: &PdfFingerprint) -> Ordering {
    left.sha256()
        .cmp(&right.sha256())
        .then_with(|| left.byte_len().cmp(&right.byte_len()))
        .then_with(|| left.modified().cmp(&right.modified()))
}

/// Checks that no two records claim the same numeric ID.
///
/// Records without an ID are ignored, since the invariant only constrains the
/// ones that have a number.
///
/// # Errors
/// Returns [`DuplicateId`] for the first repeated ID, naming both records.
pub fn validate_unique_ids<'a>(
    notes: impl Iterator<Item = &'a IndexedNote>,
) -> Result<(), DuplicateId> {
    let mut seen: HashMap<NoteId, &RelativePdfPath> = HashMap::new();
    for note in notes {
        if let Some(id) = note.id()
            && let Some(existing) = seen.insert(id, note.path())
        {
            return Err(DuplicateId {
                id,
                existing: existing.clone(),
                conflicting: note.path().clone(),
            });
        }
    }
    Ok(())
}

/// Case-folds a title for comparison, ignoring ASCII and Unicode case.
fn case_folded(title: &str) -> String {
    title.to_lowercase()
}