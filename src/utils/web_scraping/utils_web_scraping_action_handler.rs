use crate::{
    api::Api,
    users::{PublicUserDataNamespace, User, UserId},
    utils::{
        web_scraping::resources::{
            WebScraperResourceBundle, WebScraperResourcesRequest, WebScraperResourcesResponse,
        },
        UtilsWebScrapingAction, UtilsWebScrapingActionResult, WebPageResource, WebPageResources,
        WebPageResourcesTracker,
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
            } => {
                let tracker = Self::get_tracker(api, user.id, &tracker_name).await?;

                // If tracker is configured to persist resource, and client requests refresh, fetch
                // resources with the scraper and persist them.
                if tracker.revisions > 0 && refresh {
                    let bundle_to_resources =
                        |bundle: WebScraperResourceBundle| -> Vec<WebPageResource> {
                            // TODO: Return all resources, not just external ones.
                            bundle
                                .external
                                .into_iter()
                                .filter_map(|resource| {
                                    Some(WebPageResource {
                                        url: resource.url?,
                                        digest: resource.digest,
                                        size: resource.size,
                                    })
                                })
                                .collect()
                        };

                    let scraper_response = reqwest::Client::new()
                        .post(format!(
                            "{}api/resources",
                            api.config.components.web_scraper_url.as_str()
                        ))
                        .json(&WebScraperResourcesRequest::with_default_parameters(
                            &tracker.url,
                        ))
                        .send()
                        .await?
                        .json::<WebScraperResourcesResponse>()
                        .await?;

                    api.web_scraping()
                        .save_web_page_resources(
                            user.id,
                            &tracker,
                            WebPageResources {
                                timestamp: scraper_response.timestamp,
                                scripts: bundle_to_resources(scraper_response.scripts),
                                styles: bundle_to_resources(scraper_response.styles),
                            },
                        )
                        .await?;
                }

                // Retrieve latest persisted resources.
                Ok(UtilsWebScrapingActionResult::FetchWebPageResources {
                    tracker_name,
                    resources: api
                        .web_scraping()
                        .get_web_page_resources(user.id, &tracker)
                        .await?,
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
