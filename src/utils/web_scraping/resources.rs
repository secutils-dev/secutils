mod web_page_resource;
mod web_page_resource_content;
mod web_page_resource_content_data;
mod web_page_resource_diff_status;
mod web_page_resources_revision;
mod web_page_resources_revisions_diff;
mod web_page_resources_tracker;
mod web_page_resources_tracker_scripts;
mod web_page_resources_tracker_settings;
mod web_scraper_resources_request;
mod web_scraper_resources_response;

pub use self::{
    web_page_resource::WebPageResource,
    web_page_resource_content::WebPageResourceContent,
    web_page_resource_content_data::WebPageResourceContentData,
    web_page_resource_diff_status::WebPageResourceDiffStatus,
    web_page_resources_revision::WebPageResourcesRevision,
    web_page_resources_revisions_diff::web_page_resources_revisions_diff,
    web_page_resources_tracker::WebPageResourcesTracker,
    web_page_resources_tracker_scripts::WebPageResourcesTrackerScripts,
    web_page_resources_tracker_settings::{
        WebPageResourcesTrackerSettings, MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY,
        MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS,
    },
    web_scraper_resources_request::{
        WebScraperResourcesRequest, WebScraperResourcesRequestScripts,
    },
    web_scraper_resources_response::{WebScraperResource, WebScraperResourcesResponse},
};
