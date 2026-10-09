//! The default library browsing order and the unique-ID invariant.

use std::cmp::Ordering;
use std::collections::HashMap;

use thiserror::Error;

use crate::{IndexedNote, NoteId, RelativePdfPath};

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
/// Records with IDs come first, in ascending numeric order, so `2` precedes
/// `10`. Records without IDs follow, ordered by case-folded title so `Apple`
/// and `apricot` interleave as the user reads them. The library-relative path
/// breaks any remaining tie, which keeps the order total and therefore stable
/// across runs.
///
/// The tie-break chain applies to every pair, including two records that share
/// an ID. Duplicate IDs are surfaced by [`validate_unique_ids`] rather than
/// renumbered, so such records coexist in the library and must still sort
/// deterministically.
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