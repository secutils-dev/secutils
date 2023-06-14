mod web_page_resource;
mod web_page_resources_tracker;
mod web_scrapper_resources_request;
mod web_scrapper_resources_response;

pub use self::{
    web_page_resource::WebPageResource,
    web_page_resources_tracker::WebPageResourcesTracker,
    web_scrapper_resources_request::WebScrapperResourcesRequest,
    web_scrapper_resources_response::{
        WebScrapperResource, WebScrapperResourceBundle, WebScrapperResourcesResponse,
    },
};
