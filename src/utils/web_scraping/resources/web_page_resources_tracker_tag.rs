use crate::utils::{
    web_scraping::resources::WebPageResourceInternal, WebPageResourcesData, WebPageTrackerKind,
    WebPageTrackerTag,
};

/// Struct that represents a tag for the `WebPageTracker` that tracks the resources of a web page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebPageResourcesTrackerTag(());
impl WebPageTrackerTag for WebPageResourcesTrackerTag {
    const KIND: WebPageTrackerKind = WebPageTrackerKind::WebPageResources;
    type TrackerMeta = ();
    type TrackerData = WebPageResourcesData;
}

/// Internal struct that represents a tag for the `WebPageTracker` that tracks the resources of a
/// web page and that overrides `TrackerData` type with the type compatible with Postcard.
pub(in crate::utils::web_scraping) struct WebPageResourcesTrackerInternalTag(());
impl WebPageTrackerTag for WebPageResourcesTrackerInternalTag {
    const KIND: WebPageTrackerKind = WebPageResourcesTrackerTag::KIND;
    type TrackerMeta = ();
    type TrackerData = WebPageResourcesData<WebPageResourceInternal>;
}
