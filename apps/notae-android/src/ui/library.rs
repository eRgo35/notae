use notae_index::IndexStore;

use crate::ui::search::{SearchRow, UiError};

/// Search and browse projection over the local indexed library.
pub struct LibraryViewModel<'a> {
    store: &'a IndexStore,
    query: String,
}

impl<'a> LibraryViewModel<'a> {
    /// Creates a view model over an open local index.
    #[must_use]
    pub fn new(store: &'a IndexStore) -> Self {
        Self {
            store,
            query: String::new(),
        }
    }

    /// Replaces the current search text.
    pub fn set_query(&mut self, query: String) {
        self.query = query;
    }

    /// Returns ranked matches, or the full numeric-order library when the
    /// query is empty.
    pub fn search(&self) -> Result<Vec<SearchRow>, UiError> {
        let notes = if self.query.trim().is_empty() {
            self.store.list_all()?
        } else {
            self.store
                .search(&self.query, usize::MAX)?
                .into_iter()
                .map(|hit| hit.note)
                .collect()
        };
        Ok(notes.into_iter().map(SearchRow::from_note).collect())
    }
}
