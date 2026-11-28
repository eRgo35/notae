//! Notae's own viewer: which score it shows and, inside a set, where it stands among the pieces.

use crate::ui::browse::SetEntry;
use crate::ui::search::SearchRow;

/// The score on the viewer. It is drawn over the screen that opened it, which stays as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerState {
    /// The row as it was when opened; the snapshot refreshes it from the library when it can.
    pub row: SearchRow,
    /// The set the score was opened from, with `row.item_id` its item.
    pub set_id: Option<String>,
}

/// Where a set item stands among the set's pieces: the pieces before and after it and its
/// one-based position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPlace {
    /// The piece before it.
    pub previous: Option<SearchRow>,
    /// The piece after it.
    pub next: Option<SearchRow>,
    /// Its one-based position.
    pub position: usize,
    /// The pieces counted.
    pub total: usize,
}

/// Where set item `item_id` stands among the set's pieces, skipping text notes and pieces whose
/// PDF is not in the library; `None` when the set does not hold it.
#[must_use]
pub fn place(entries: &[SetEntry], item_id: &str) -> Option<SetPlace> {
    let pieces: Vec<&SearchRow> = entries
        .iter()
        .filter_map(|entry| match entry {
            SetEntry::Piece(row) if !row.missing || row.item_id.as_deref() == Some(item_id) => {
                Some(row)
            }
            _ => None,
        })
        .collect();
    let index = pieces
        .iter()
        .position(|row| row.item_id.as_deref() == Some(item_id))?;
    Some(SetPlace {
        previous: index
            .checked_sub(1)
            .and_then(|before| pieces.get(before).map(|row| (*row).clone())),
        next: pieces.get(index + 1).map(|row| (*row).clone()),
        position: index + 1,
        total: pieces.len(),
    })
}
