mod web_page_resource;
mod web_page_resources;
mod web_page_resources_tracker;
mod web_scraper_resources_request;
mod web_scraper_resources_response;

pub use self::{
    web_page_resource::WebPageResource,
    web_page_resources::WebPageResources,
    web_page_resources_tracker::WebPageResourcesTracker,
    web_scraper_resources_request::WebScraperResourcesRequest,
    web_scraper_resources_response::{WebScraperResource, WebScraperResourcesResponse},
};
