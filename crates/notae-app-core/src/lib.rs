//! UI-toolkit-free app logic for the Notae score library.
//!
//! [`UiModel`] owns everything a screen shows: the [`AppController`] state machine, the library
//! storage, favorites, sets, recent scores, preferences and the thumbnail cache. The
//! [`operations`] worker rescans the library off the UI thread, and [`backup`] reads and writes
//! the export file. Nothing here depends on a UI toolkit or on Android, so the whole crate is
//! host-testable; the UI that renders it lives in the app crates. The core builds no display
//! text: titles, counts, dates and statuses come back as the [`text`] types, and each UI
//! formats them in its own language.

#![forbid(unsafe_code)]

pub mod backup;
pub mod favorites;
pub mod operations;
pub mod pdf_intent;
pub mod pdf_pages;
pub mod platform;
pub mod recent;
pub mod sets;
pub mod state;
pub mod storage;
pub mod text;
pub mod thumbnails;
pub mod ui;
pub mod ui_prefs;

pub use favorites::Favorites;
pub use sets::SetStore;
pub use state::{AppController, AppState, Screen, Setup};
pub use ui::UiModel;
pub use ui::browse::LibrarySection;
pub use ui::search::SearchRow;
