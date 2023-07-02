mod web_page_resource;
mod web_page_resource_content;
mod web_page_resource_diff_status;
mod web_page_resources_revision;
mod web_page_resources_revisions_diff;
mod web_page_resources_tracker;
mod web_scraper_resources_request;
mod web_scraper_resources_response;

pub use self::{
    web_page_resource::WebPageResource,
    web_page_resource_content::WebPageResourceContent,
    web_page_resource_diff_status::WebPageResourceDiffStatus,
    web_page_resources_revision::WebPageResourcesRevision,
    web_page_resources_revisions_diff::web_page_resources_revisions_diff,
    web_page_resources_tracker::WebPageResourcesTracker,
    web_scraper_resources_request::WebScraperResourcesRequest,
    web_scraper_resources_response::{
        WebScraperResource, WebScraperResourceContent, WebScraperResourcesResponse,
    },
};
