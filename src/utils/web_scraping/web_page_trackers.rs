mod web_page_content;
mod web_page_data_revision;
mod web_page_resources;
mod web_page_tracker;
mod web_page_tracker_kind;
mod web_page_tracker_settings;
mod web_page_tracker_tag;
mod web_scraper;

pub use self::{
    web_page_content::{
        WebPageContentTrackerTag, WebScraperContentRequest, WebScraperContentRequestScripts,
        WebScraperContentResponse, web_page_content_revisions_diff,
    },
    web_page_data_revision::WebPageDataRevision,
    web_page_resources::{
        WebPageResource, WebPageResourceContent, WebPageResourceContentData,
        WebPageResourceDiffStatus, WebPageResourcesData, WebPageResourcesTrackerTag,
        WebScraperResource, WebScraperResourcesRequest, WebScraperResourcesRequestScripts,
        WebScraperResourcesResponse, web_page_resources_revisions_diff,
    },
    web_page_tracker::WebPageTracker,
    web_page_tracker_kind::WebPageTrackerKind,
    web_page_tracker_settings::WebPageTrackerSettings,
    web_page_tracker_tag::WebPageTrackerTag,
    web_scraper::WebScraperErrorResponse,
};

pub(in crate::utils::web_scraping) use self::web_page_resources::{
    WebPageResourceInternal, WebPageResourcesTrackerInternalTag,
};
