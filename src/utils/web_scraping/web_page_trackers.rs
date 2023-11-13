mod web_page_data_revision;
mod web_page_resources;
mod web_page_tracker;
mod web_page_tracker_kind;
mod web_page_tracker_settings;
mod web_page_tracker_tag;

pub use self::{
    web_page_data_revision::WebPageDataRevision,
    web_page_resources::{
        web_page_resources_revisions_diff, WebPageResource, WebPageResourceContent,
        WebPageResourceContentData, WebPageResourceDiffStatus, WebPageResourcesData,
        WebPageResourcesTrackerTag, WebScraperResource, WebScraperResourcesRequest,
        WebScraperResourcesRequestScripts, WebScraperResourcesResponse,
    },
    web_page_tracker::WebPageTracker,
    web_page_tracker_kind::WebPageTrackerKind,
    web_page_tracker_settings::{
        WebPageTrackerSettings, MAX_WEB_PAGE_TRACKER_DELAY, MAX_WEB_PAGE_TRACKER_REVISIONS,
    },
    web_page_tracker_tag::WebPageTrackerTag,
};

pub(in crate::utils::web_scraping) use self::web_page_resources::{
    WebPageResourceInternal, WebPageResourcesTrackerInternalTag,
};
