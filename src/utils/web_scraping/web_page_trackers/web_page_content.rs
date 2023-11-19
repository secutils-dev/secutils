mod web_page_content_tracker_tag;
mod web_scraper_content_request;
mod web_scraper_content_response;

pub use self::{
    web_page_content_tracker_tag::WebPageContentTrackerTag,
    web_scraper_content_request::{WebScraperContentRequest, WebScraperContentRequestScripts},
    web_scraper_content_response::WebScraperContentResponse,
};
