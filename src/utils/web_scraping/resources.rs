mod web_page_resource;
mod web_page_resource_content;
mod web_page_resource_content_data;
mod web_page_resource_diff_status;
mod web_page_resources_data;
mod web_page_resources_revisions_diff;
mod web_page_resources_tracker_tag;
mod web_scraper_resources_request;
mod web_scraper_resources_response;

pub use self::{
    web_page_resource::WebPageResource,
    web_page_resource_content::WebPageResourceContent,
    web_page_resource_content_data::WebPageResourceContentData,
    web_page_resource_diff_status::WebPageResourceDiffStatus,
    web_page_resources_data::WebPageResourcesData,
    web_page_resources_revisions_diff::web_page_resources_revisions_diff,
    web_page_resources_tracker_tag::WebPageResourcesTrackerTag,
    web_scraper_resources_request::{
        WebScraperResourcesRequest, WebScraperResourcesRequestScripts,
    },
    web_scraper_resources_response::{WebScraperResource, WebScraperResourcesResponse},
};

pub(in crate::utils::web_scraping) use self::{
    web_page_resource::WebPageResourceInternal,
    web_page_resources_tracker_tag::WebPageResourcesTrackerInternalTag,
};
