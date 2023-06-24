mod api;
mod resources;
mod utils_web_scraping_action;
mod utils_web_scraping_action_handler;
mod utils_web_scraping_action_result;

pub use self::{
    api::WebScrapingApi,
    resources::{
        WebPageResource, WebPageResourceContent, WebPageResources, WebPageResourcesTracker,
    },
    utils_web_scraping_action::UtilsWebScrapingAction,
    utils_web_scraping_action_handler::UtilsWebScrapingActionHandler,
    utils_web_scraping_action_result::UtilsWebScrapingActionResult,
};
