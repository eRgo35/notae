use notae_index::{IndexStore, SearchScope};

use crate::ui::search::{SearchRow, UiError};

/// Search and browse projection over the local indexed library.
pub struct LibraryViewModel<'a> {
    store: &'a IndexStore,
    query: String,
    scope: SearchScope,
}

impl<'a> LibraryViewModel<'a> {
    /// Creates a view model over an open local index.
    #[must_use]
    pub fn new(store: &'a IndexStore) -> Self {
        Self {
            store,
            query: String::new(),
            scope: SearchScope::default(),
        }
    }

    /// Replaces the current search text.
    pub fn set_query(&mut self, query: String) {
        self.query = query;
    }

    /// Restricts the next search to `scope`.
    pub fn set_scope(&mut self, scope: SearchScope) {
        self.scope = scope;
    }

    /// Returns ranked matches, or the full numeric-order library when the
    /// query is empty.
    pub fn search(&self) -> Result<Vec<SearchRow>, UiError> {
        if self.query.trim().is_empty() {
            return Ok(self.store.list_all()?.into_iter().map(SearchRow::from_note).collect());
        }
        Ok(self
            .store
            .search_in(&self.query, self.scope, usize::MAX)?
            .into_iter()
            .map(|hit| {
                let mut row = SearchRow::from_note(hit.note);
                row.text_snippet = hit.text_snippet;
                row
            })
            .collect())
    }
}
