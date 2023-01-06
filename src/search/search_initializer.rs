use crate::{api::Api, search::SearchItem};
use time::OffsetDateTime;

pub fn search_initializer(api: &Api) -> anyhow::Result<()> {
    // TODO: Implement real search index initialization.
    api.search().upsert(&SearchItem {
        id: "search-id-test".to_string(),
        content: "search content test".to_string(),
        timestamp: OffsetDateTime::now_utc(),
        user_handle: Some("handle".to_string()),
    })
}
