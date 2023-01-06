use serde_derive::Serialize;
use time::OffsetDateTime;

#[derive(Serialize, Debug, Eq, PartialEq, Clone)]
pub struct SearchItem {
    pub id: String,
    pub content: String,
    pub user_handle: Option<String>,
    #[serde(with = "time::serde::timestamp")]
    pub timestamp: OffsetDateTime,
}

impl AsRef<SearchItem> for SearchItem {
    fn as_ref(&self) -> &SearchItem {
        self
    }
}
