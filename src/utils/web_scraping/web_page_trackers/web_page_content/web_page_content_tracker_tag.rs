use crate::utils::web_scraping::{WebPageTrackerKind, WebPageTrackerTag};

/// Struct that represents a tag for the `WebPageTracker` that tracks the content of a web page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebPageContentTrackerTag(());
impl WebPageTrackerTag for WebPageContentTrackerTag {
    const KIND: WebPageTrackerKind = WebPageTrackerKind::WebPageContent;
    type TrackerMeta = ();
    type TrackerData = String;
}
