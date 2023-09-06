mod api_ext;
mod resources;
mod utils_web_scraping_action;
mod utils_web_scraping_action_result;

pub use self::{
    api_ext::WebScrapingApi,
    resources::{
        web_page_resources_revisions_diff, WebPageResource, WebPageResourceContent,
        WebPageResourceContentData, WebPageResourceDiffStatus, WebPageResourcesRevision,
        WebPageResourcesTracker, WebPageResourcesTrackerScripts, WebScraperResource,
        WebScraperResourcesRequest, WebScraperResourcesRequestScripts, WebScraperResourcesResponse,
        MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY, MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS,
    },
    utils_web_scraping_action::UtilsWebScrapingAction,
    utils_web_scraping_action_result::UtilsWebScrapingActionResult,
};

#[cfg(test)]
pub mod tests {
    use crate::utils::{
        web_scraping::resources::WebPageResourcesTrackerScripts, WebPageResourcesTracker,
    };
    use std::time::Duration;
    use url::Url;

    pub struct MockWebPageResourcesTrackerBuilder {
        tracker: WebPageResourcesTracker,
    }

    impl MockWebPageResourcesTrackerBuilder {
        pub fn create<N: Into<String>>(
            name: N,
            url: &str,
            revisions: usize,
        ) -> anyhow::Result<Self> {
            Ok(Self {
                tracker: WebPageResourcesTracker {
                    name: name.into(),
                    url: Url::parse(url)?,
                    revisions,
                    delay: Duration::from_millis(2000),
                    schedule: None,
                    scripts: Default::default(),
                },
            })
        }

        pub fn with_schedule<S: Into<String>>(mut self, schedule: S) -> Self {
            self.tracker.schedule = Some(schedule.into());
            self
        }

        pub fn with_delay_millis(mut self, millis: u64) -> Self {
            self.tracker.delay = Duration::from_millis(millis);
            self
        }

        pub fn with_scripts(mut self, scripts: WebPageResourcesTrackerScripts) -> Self {
            self.tracker.scripts = scripts;
            self
        }

        pub fn build(self) -> WebPageResourcesTracker {
            self.tracker
        }
    }
}
