use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HomeSummary {
    pub counts: HomeSummaryCounts,
    pub recent_items: Vec<HomeSummaryRecentItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HomeSummaryCounts {
    pub webhooks: i64,
    pub certificates: i64,
    pub csp: i64,
    pub web_scraping: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HomeSummaryRecentItem {
    pub name: String,
    pub util_handle: String,
    #[serde(with = "time::serde::timestamp")]
    pub updated_at: OffsetDateTime,
}
