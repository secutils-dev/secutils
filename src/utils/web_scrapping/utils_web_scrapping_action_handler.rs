use crate::{
    api::Api,
    users::{PublicUserDataNamespace, User},
    utils::{UtilsWebScrappingAction, UtilsWebScrappingActionResult, WebPageResourcesTracker},
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

                Ok(UtilsWebScrappingActionResult::TrackWebPageResources {
                    tracker_name: tracker.name,
                    resources: vec![],
                })
            }
        }
    }
}
