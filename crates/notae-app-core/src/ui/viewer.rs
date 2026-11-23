//! Notae's own viewer: which score it shows and, inside a set, the pieces before and after it.

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

/// The pieces before and after set item `item_id`, skipping text notes and pieces whose PDF is
/// not in the library.
#[must_use]
pub fn neighbours<'a>(
    entries: &'a [SetEntry],
    item_id: &str,
) -> (Option<&'a SearchRow>, Option<&'a SearchRow>) {
    let pieces: Vec<&SearchRow> = entries
        .iter()
        .filter_map(|entry| match entry {
            SetEntry::Piece(row) if !row.missing || row.item_id.as_deref() == Some(item_id) => {
                Some(row)
            }
            _ => None,
        })
        .collect();
    let Some(index) = pieces
        .iter()
        .position(|row| row.item_id.as_deref() == Some(item_id))
    else {
        return (None, None);
    };
    (
        index
            .checked_sub(1)
            .and_then(|before| pieces.get(before).copied()),
        pieces.get(index + 1).copied(),
    )
}
