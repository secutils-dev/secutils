use crate::{
    api::Api,
    users::{PublicUserDataNamespace, User, UserId},
    utils::{
        web_scraping::resources::{
            web_page_resources_revisions_diff, WebScraperResource, WebScraperResourcesRequest,
            WebScraperResourcesResponse,
        },
        UtilsWebScrapingAction, UtilsWebScrapingActionResult, WebPageResource,
        WebPageResourcesRevision, WebPageResourcesTracker,
    },
};
use anyhow::anyhow;
use std::collections::BTreeMap;

pub struct UtilsWebScrapingActionHandler;
impl UtilsWebScrapingActionHandler {
    pub async fn handle(
        user: User,
        api: &Api,
        action: UtilsWebScrapingAction,
    ) -> anyhow::Result<UtilsWebScrapingActionResult> {
        match action {
            UtilsWebScrapingAction::SaveWebPageResourcesTracker { tracker } => {
                Ok(UtilsWebScrapingActionResult::SaveWebPageResourcesTracker {
                    tracker: api
                        .web_scraping()
                        .save_web_page_resources_tracker(user.id, tracker)
                        .await?,
                })
            }
            UtilsWebScrapingAction::RemoveWebPageResourcesTracker { tracker_name } => {
                api.web_scraping()
                    .remove_web_page_resources_tracker(user.id, &tracker_name)
                    .await?;
                Ok(UtilsWebScrapingActionResult::RemoveWebPageResourcesTracker)
            }
            UtilsWebScrapingAction::FetchWebPageResources {
                tracker_name,
                refresh,
                calculate_diff,
            } => {
                let tracker = Self::get_tracker(api, user.id, &tracker_name).await?;

                // If tracker is configured to persist resource, and client requests refresh, fetch
                // resources with the scraper and persist them.
                if tracker.revisions > 0 && refresh {
                    let convert_to_web_page_resources =
                        |resources: Vec<WebScraperResource>| -> Vec<WebPageResource> {
                            resources
                                .into_iter()
                                .map(|resource| resource.into())
                                .collect()
                        };

                    let scraper_response = reqwest::Client::new()
                        .post(format!(
                            "{}api/resources",
                            api.config.components.web_scraper_url.as_str()
                        ))
                        .json(
                            &WebScraperResourcesRequest::with_default_parameters(&tracker.url)
                                .set_delay(tracker.delay),
                        )
                        .send()
                        .await?
                        .json::<WebScraperResourcesResponse>()
                        .await?;

                    api.web_scraping()
                        .save_web_page_resources(
                            user.id,
                            &tracker,
                            WebPageResourcesRevision {
                                timestamp: scraper_response.timestamp,
                                scripts: convert_to_web_page_resources(scraper_response.scripts),
                                styles: convert_to_web_page_resources(scraper_response.styles),
                            },
                        )
                        .await?;
                }

                let revisions = api
                    .web_scraping()
                    .get_web_page_resources(user.id, &tracker)
                    .await?;

                // Retrieve latest persisted resources.
                Ok(UtilsWebScrapingActionResult::FetchWebPageResources {
                    tracker_name,
                    revisions: if calculate_diff {
                        web_page_resources_revisions_diff(revisions)?
                    } else {
                        revisions
                    },
                })
            }
            UtilsWebScrapingAction::RemoveWebPageResources { tracker_name } => {
                api.web_scraping()
                    .remove_tracked_web_page_resources(
                        user.id,
                        &Self::get_tracker(api, user.id, &tracker_name).await?,
                    )
                    .await?;

                Ok(UtilsWebScrapingActionResult::RemoveWebPageResources)
            }
        }
    }

    async fn get_tracker(
        api: &Api,
        user_id: UserId,
        tracker_name: &str,
    ) -> anyhow::Result<WebPageResourcesTracker> {
        api.users()
            .get_data::<BTreeMap<String, WebPageResourcesTracker>>(
                user_id,
                PublicUserDataNamespace::WebPageResourcesTrackers,
            )
            .await?
            .and_then(|mut map| map.value.remove(tracker_name))
            .ok_or_else(|| {
                anyhow!(
                    "Cannot find web page resources tracker with name: {}",
                    tracker_name
                )
            })
    }
}
