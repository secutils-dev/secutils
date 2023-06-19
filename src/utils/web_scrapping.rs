mod api;
mod resources;
mod utils_web_scrapping_action;
mod utils_web_scrapping_action_handler;
mod utils_web_scrapping_action_result;

pub use self::{
    api::WebScrappingApi,
    resources::{WebPageResource, WebPageResources, WebPageResourcesTracker},
    utils_web_scrapping_action::UtilsWebScrappingAction,
    utils_web_scrapping_action_handler::UtilsWebScrappingActionHandler,
    utils_web_scrapping_action_result::UtilsWebScrappingActionResult,
};
