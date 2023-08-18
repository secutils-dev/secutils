mod api;
mod resources;
mod utils_web_scraping_action;
mod utils_web_scraping_action_result;

pub use self::{
    api::WebScrapingApi,
    resources::{
        WebPageResource, WebPageResourceContent, WebPageResourceContentData,
        WebPageResourceDiffStatus, WebPageResourcesRevision, WebPageResourcesTracker,
        WebScraperResource, WebScraperResourcesRequest, WebScraperResourcesResponse,
        MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY, MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS,
    },
    utils_web_scraping_action::UtilsWebScrapingAction,
    utils_web_scraping_action_result::UtilsWebScrapingActionResult,
};
