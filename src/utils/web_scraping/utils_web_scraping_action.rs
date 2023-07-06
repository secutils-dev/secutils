use crate::utils::WebPageResourcesTracker;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebScrapingAction {
    #[serde(rename_all = "camelCase")]
    FetchWebPageResources {
        tracker_name: String,
        #[serde(default)]
        refresh: bool,
        #[serde(default)]
        calculate_diff: bool,
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
    use crate::utils::{UtilsWebScrapingAction, WebPageResourcesTracker};
    use std::time::Duration;
    use url::Url;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r###"
    {
        "type": "fetchWebPageResources",
        "value": { "trackerName": "tracker" }
    }
              "###
            )?,
            UtilsWebScrapingAction::FetchWebPageResources {
                tracker_name: "tracker".to_string(),
                refresh: false,
                calculate_diff: false
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r###"
    {
        "type": "fetchWebPageResources",
        "value": { "trackerName": "tracker", "refresh": true, "calculateDiff": true }
    }
              "###
            )?,
            UtilsWebScrapingAction::FetchWebPageResources {
                tracker_name: "tracker".to_string(),
                refresh: true,
                calculate_diff: true
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r###"
    {
        "type": "removeWebPageResources",
        "value": { "trackerName": "tracker" }
    }
              "###
            )?,
            UtilsWebScrapingAction::RemoveWebPageResources {
                tracker_name: "tracker".to_string(),
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r###"
    {
        "type": "saveWebPageResourcesTracker",
        "value": { "tracker": { "name": "some-name", "url": "http://localhost:1234/my/app?q=2", "revisions": 3, "delay": 2000 } }
    }
              "###
            )?,
            UtilsWebScrapingAction::SaveWebPageResourcesTracker {
                tracker: WebPageResourcesTracker {
                    name: "some-name".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                }
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r###"
    {
        "type": "removeWebPageResourcesTracker",
        "value": { "trackerName": "tracker" }
    }
              "###
            )?,
            UtilsWebScrapingAction::RemoveWebPageResourcesTracker {
                tracker_name: "tracker".to_string(),
            }
        );

        Ok(())
    }
}
