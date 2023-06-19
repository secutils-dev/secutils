use crate::utils::WebPageResourcesTracker;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebScrappingAction {
    #[serde(rename_all = "camelCase")]
    FetchWebPageResources {
        tracker_name: String,
        #[serde(default)]
        refresh: bool,
    },
    #[serde(rename_all = "camelCase")]
    RemoveWebPageResources { tracker_name: String },
    #[serde(rename_all = "camelCase")]
    SaveWebPageResourcesTracker { tracker: WebPageResourcesTracker },
    #[serde(rename_all = "camelCase")]
    RemoveWebPageResourcesTracker { tracker_name: String },
}

#[cfg(test)]
mod tests {
    use crate::utils::{UtilsWebScrappingAction, WebPageResourcesTracker};
    use url::Url;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UtilsWebScrappingAction>(
                r###"
    {
        "type": "fetchWebPageResources",
        "value": { "trackerName": "tracker" }
    }
              "###
            )?,
            UtilsWebScrappingAction::FetchWebPageResources {
                tracker_name: "tracker".to_string(),
                refresh: false
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrappingAction>(
                r###"
    {
        "type": "fetchWebPageResources",
        "value": { "trackerName": "tracker", "refresh": true }
    }
              "###
            )?,
            UtilsWebScrappingAction::FetchWebPageResources {
                tracker_name: "tracker".to_string(),
                refresh: true
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrappingAction>(
                r###"
    {
        "type": "removeWebPageResources",
        "value": { "trackerName": "tracker" }
    }
              "###
            )?,
            UtilsWebScrappingAction::RemoveWebPageResources {
                tracker_name: "tracker".to_string(),
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrappingAction>(
                r###"
    {
        "type": "saveWebPageResourcesTracker",
        "value": { "tracker": { "name": "some-name", "url": "http://localhost:1234/my/app?q=2", "revisions": 3 } }
    }
              "###
            )?,
            UtilsWebScrappingAction::SaveWebPageResourcesTracker {
                tracker: WebPageResourcesTracker {
                    name: "some-name".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 3
                }
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrappingAction>(
                r###"
    {
        "type": "removeWebPageResourcesTracker",
        "value": { "trackerName": "tracker" }
    }
              "###
            )?,
            UtilsWebScrappingAction::RemoveWebPageResourcesTracker {
                tracker_name: "tracker".to_string(),
            }
        );

        Ok(())
    }
}
