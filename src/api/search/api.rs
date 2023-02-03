use crate::{
    datastore::{SearchFilter, SearchIndex},
    search::SearchItem,
};
use std::borrow::Cow;

/// API to facilitate interaction with the application-wide search.
pub struct SearchApi<'a> {
    search_index: Cow<'a, SearchIndex>,
}

impl<'a> SearchApi<'a> {
    /// Creates Search API.
    pub fn new(search_index: &'a SearchIndex) -> Self {
        Self {
            search_index: Cow::Borrowed(search_index),
        }
    }

    /// Search using the specified query.
    pub fn search(&self, filter: SearchFilter<'_, '_>) -> anyhow::Result<Vec<SearchItem>> {
        self.search_index.search(filter)
    }

    /// Adds or updates a search item.
    pub fn upsert<I: AsRef<SearchItem>>(&self, item: I) -> anyhow::Result<()> {
        self.search_index.upsert(item)
    }

    /// Removes a search item with the specific id.
    pub fn remove(&self, id: u64) -> anyhow::Result<()> {
        self.search_index.remove(id)
    }
}
