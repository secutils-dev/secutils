use crate::{
    api::Api,
    users::{PublicUserDataNamespace, User, UserId},
    utils::{
        web_scrapping::resources::{
            WebScrapperResourceBundle, WebScrapperResourcesRequest, WebScrapperResourcesResponse,
        },
        UtilsWebScrappingAction, UtilsWebScrappingActionResult, WebPageResource, WebPageResources,
        WebPageResourcesTracker,
    },
};
use anyhow::anyhow;
use std::collections::BTreeMap;

pub struct UtilsWebScrappingActionHandler;
impl UtilsWebScrappingActionHandler {
    pub async fn handle(
        user: User,
        api: &Api,
        action: UtilsWebScrappingAction,
    ) -> anyhow::Result<UtilsWebScrappingActionResult> {
        match action {
            UtilsWebScrappingAction::SaveWebPageResourcesTracker { tracker } => {
                Ok(UtilsWebScrappingActionResult::SaveWebPageResourcesTracker {
                    tracker: api
                        .web_scrapping()
                        .save_web_page_resources_tracker(user.id, tracker)
                        .await?,
                })
            }
            UtilsWebScrappingAction::RemoveWebPageResourcesTracker { tracker_name } => {
                api.web_scrapping()
                    .remove_web_page_resources_tracker(user.id, &tracker_name)
                    .await?;
                Ok(UtilsWebScrappingActionResult::RemoveWebPageResourcesTracker)
            }
            UtilsWebScrappingAction::FetchWebPageResources {
                tracker_name,
                refresh,
            } => {
                let tracker = Self::get_tracker(api, user.id, &tracker_name).await?;

                // If tracker is configured to persist resource, and client requests refresh, fetch
                // resources with the scrapper and persist them.
                if tracker.revisions > 0 && refresh {
                    let bundle_to_resources =
                        |bundle: WebScrapperResourceBundle| -> Vec<WebPageResource> {
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

                    let scrapper_response = reqwest::Client::new()
                        .post(format!(
                            "{}api/resources",
                            api.config.components.web_scrapper_url.as_str()
                        ))
                        .json(&WebScrapperResourcesRequest::with_default_parameters(
                            &tracker.url,
                        ))
                        .send()
                        .await?
                        .json::<WebScrapperResourcesResponse>()
                        .await?;

                    api.web_scrapping()
                        .save_web_page_resources(
                            user.id,
                            &tracker,
                            WebPageResources {
                                timestamp: scrapper_response.timestamp,
                                scripts: bundle_to_resources(scrapper_response.scripts),
                                styles: bundle_to_resources(scrapper_response.styles),
                            },
                        )
                        .await?;
                }

                // Retrieve latest persisted resources.
                Ok(UtilsWebScrappingActionResult::FetchWebPageResources {
                    tracker_name,
                    resources: api
                        .web_scrapping()
                        .get_web_page_resources(user.id, &tracker)
                        .await?,
                })
            }
            UtilsWebScrappingAction::RemoveWebPageResources { tracker_name } => {
                api.web_scrapping()
                    .remove_tracked_web_page_resources(
                        user.id,
                        &Self::get_tracker(api, user.id, &tracker_name).await?,
                    )
                    .await?;

                Ok(UtilsWebScrappingActionResult::RemoveWebPageResources)
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
