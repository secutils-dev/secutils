use crate::{api::Api, search::SearchItem};
use time::OffsetDateTime;

pub fn search_initializer(api: &Api) -> anyhow::Result<()> {
    // TODO: Implement real search index initialization.
    api.search().upsert(&SearchItem {
        id: SearchItem::create_id("Responders", "Utils", Some("Webhooks"), None),
        label: "Responders".to_string(),
        keywords: Some("webhooks http request response".to_string()),
        category: "Utils".to_string(),
        sub_category: Some("Webhooks".to_string()),
        user_id: None,
        timestamp: OffsetDateTime::now_utc(),
        meta: None,
    })
}
