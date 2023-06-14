use crate::{
    api::Api,
    users::{PublicUserDataNamespace, User},
    utils::{
        web_scrapping::resources::{WebScrapperResourcesRequest, WebScrapperResourcesResponse},
        UtilsWebScrappingAction, UtilsWebScrappingActionResult, WebPageResource,
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
            UtilsWebScrappingAction::TrackWebPageResources { tracker_name } => {
                let tracker = api
                    .users()
                    .get_data::<BTreeMap<String, WebPageResourcesTracker>>(
                        user.id,
                        PublicUserDataNamespace::WebPageResourcesTrackers,
                    )
                    .await?
                    .and_then(|mut map| map.value.remove(&tracker_name))
                    .ok_or_else(|| {
                        anyhow!(
                            "Cannot find web page resources tracker with name: {}",
                            tracker_name
                        )
                    })?;

                let web_scrapper_response = reqwest::Client::new()
                    .post(format!(
                        "{}api/resources",
                        api.config.components.web_scrapper_url.as_str()
                    ))
                    .json(&WebScrapperResourcesRequest::with_default_parameters(
                        &tracker.web_page_url,
                    ))
                    .send()
                    .await?
                    .json::<WebScrapperResourcesResponse>()
                    .await?;

                // TODO: Return all resources, not just external ones.
                Ok(UtilsWebScrappingActionResult::TrackWebPageResources {
                    tracker_name: tracker.name,
                    resources: web_scrapper_response
                        .scripts
                        .external
                        .into_iter()
                        .chain(web_scrapper_response.styles.external.into_iter())
                        .filter_map(|resource| resource.url)
                        .map(|src| WebPageResource { url: src })
                        .collect(),
                })
            }
        }
    }
}
