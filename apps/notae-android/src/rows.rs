//! Projections of the core's view rows onto the Slint-generated structs.
//!
//! The core builds plain Rust rows; this module is the only place that turns
//! them into the `SharedString`-based structs `app.slint` declares.

#![forbid(unsafe_code)]

use notae_app_core::ui::search::{SearchRow, split_snippet};

/// Projects a search result onto the Slint row the Library and Search screens show.
///
/// `android_main` calls this for every row it publishes; host tests call it to
/// drive the generated UI with the real projection.
pub fn library_row(
    row: SearchRow,
    thumbnail: Option<slint::Image>,
    favorite: bool,
) -> crate::LibraryRow {
    let (before, hit, after) = row
        .text_snippet
        .as_deref()
        .map(split_snippet)
        .unwrap_or_default();
    crate::LibraryRow {
        id: row.id.unwrap_or_default().into(),
        title: row.title.into(),
        composer_or_artist: row.composer_or_artist.unwrap_or_default().into(),
        tags: row.tags.join(", ").into(),
        // An image-only PDF is searchable by title and metadata; the library does not nag about it.
        status: if row.text_pending {
            String::new()
        } else {
            row.status
        }
        .into(),
        note_key: row.note_key.into(),
        relative_path: row.relative_path.into(),
        has_thumbnail: thumbnail.is_some(),
        thumbnail: thumbnail.unwrap_or_default(),
        title_origin: row.title_origin.into(),
        composer_origin: row.composer_origin.into(),
        snippet_before: before.into(),
        snippet_match: hit.into(),
        snippet_after: after.into(),
        favorite,
        item_id: row.item_id.unwrap_or_default().into(),
        position: row
            .position
            .map_or_else(String::new, |position| position.to_string())
            .into(),
        missing: row.missing,
        just_opened: false,
    }
}

impl From<notae_app_core::ui::browse::SetCard> for crate::SetCard {
    fn from(card: notae_app_core::ui::browse::SetCard) -> Self {
        Self {
            id: card.id.into(),
            name: card.name.into(),
            date: card.date.into(),
            count: card.count.into(),
            edited: card.edited.into(),
        }
    }
}

impl From<notae_app_core::ui::SetChoice> for crate::SetChoice {
    fn from(choice: notae_app_core::ui::SetChoice) -> Self {
        Self {
            id: choice.id.into(),
            name: choice.name.into(),
            detail: choice.detail.into(),
            checked: choice.checked,
        }
    }
}

impl From<notae_app_core::ui::PickerRow> for crate::PickerRow {
    fn from(row: notae_app_core::ui::PickerRow) -> Self {
        Self {
            note_key: row.note_key.into(),
            title: row.title.into(),
            detail: row.detail.into(),
            in_set: row.in_set,
            added: row.added,
        }
    }
}
