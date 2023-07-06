use crate::utils::{
    utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH,
    web_scraping::{
        MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY, MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS,
    },
    WebPageResourcesTracker,
};
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

impl UtilsWebScrapingAction {
    /// Validates action parameters and throws if action parameters aren't valid.
    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            UtilsWebScrapingAction::FetchWebPageResources { tracker_name, .. }
            | UtilsWebScrapingAction::RemoveWebPageResources { tracker_name, .. }
            | UtilsWebScrapingAction::RemoveWebPageResourcesTracker { tracker_name } => {
                if tracker_name.is_empty() {
                    anyhow::bail!("Tracker name cannot be empty");
                }

                if tracker_name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
                    anyhow::bail!(
                        "Tracker name cannot be longer than {} characters",
                        MAX_UTILS_ENTITY_NAME_LENGTH
                    );
                }
            }
            UtilsWebScrapingAction::SaveWebPageResourcesTracker { tracker } => {
                if tracker.name.is_empty() {
                    anyhow::bail!("Tracker name cannot be empty");
                }

                if tracker.name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
                    anyhow::bail!(
                        "Tracker name cannot be longer than {} characters",
                        MAX_UTILS_ENTITY_NAME_LENGTH
                    );
                }

                if tracker.revisions > MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS {
                    anyhow::bail!(
                        "Tracker revisions count cannot be greater than {}",
                        MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS
                    );
                }

                if tracker.delay > MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY {
                    anyhow::bail!(
                        "Tracker delay cannot be greater than {}ms",
                        MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY.as_millis()
                    );
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{UtilsWebScrapingAction, WebPageResourcesTracker};
    use insta::assert_debug_snapshot;
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

    #[test]
    fn validation() -> anyhow::Result<()> {
        assert!(UtilsWebScrapingAction::FetchWebPageResources {
            tracker_name: "a".repeat(100),
            refresh: false,
            calculate_diff: false
        }
        .validate()
        .is_ok());

        assert!(UtilsWebScrapingAction::RemoveWebPageResources {
            tracker_name: "a".repeat(100),
        }
        .validate()
        .is_ok());

        assert!(UtilsWebScrapingAction::RemoveWebPageResourcesTracker {
            tracker_name: "a".repeat(100),
        }
        .validate()
        .is_ok());

        assert!(UtilsWebScrapingAction::FetchWebPageResources {
            tracker_name: "a".repeat(100),
            refresh: false,
            calculate_diff: false
        }
        .validate()
        .is_ok());

        assert!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "a".repeat(100),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                revisions: 10,
                delay: Duration::from_millis(60000),
            }
        }
        .validate()
        .is_ok());

        assert!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "a".repeat(100),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                revisions: 0,
                delay: Duration::from_millis(0),
            }
        }
        .validate()
        .is_ok());

        assert_debug_snapshot!(UtilsWebScrapingAction::FetchWebPageResources {
            tracker_name: "".to_string(),
            refresh: false,
            calculate_diff: false
        }
        .validate(), @r###"
        Err(
            "Tracker name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::FetchWebPageResources {
            tracker_name: "a".repeat(101),
            refresh: false,
            calculate_diff: false
        }
        .validate(), @r###"
        Err(
            "Tracker name cannot be longer than 100 characters",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::RemoveWebPageResources {
            tracker_name: "".to_string(),
        }
        .validate(), @r###"
        Err(
            "Tracker name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::RemoveWebPageResources {
            tracker_name: "a".repeat(101),
        }
        .validate(), @r###"
        Err(
            "Tracker name cannot be longer than 100 characters",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::RemoveWebPageResourcesTracker {
            tracker_name: "".to_string(),
        }
        .validate(), @r###"
        Err(
            "Tracker name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::RemoveWebPageResourcesTracker {
            tracker_name: "a".repeat(101),
        }
        .validate(), @r###"
        Err(
            "Tracker name cannot be longer than 100 characters",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                revisions: 3,
                delay: Duration::from_millis(2000),
            }
        }
        .validate(), @r###"
        Err(
            "Tracker name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "a".repeat(101),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                revisions: 3,
                delay: Duration::from_millis(2000),
            }
        }
        .validate(), @r###"
        Err(
            "Tracker name cannot be longer than 100 characters",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "a".repeat(100),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                revisions: 11,
                delay: Duration::from_millis(2000),
            }
        }
        .validate(), @r###"
        Err(
            "Tracker revisions count cannot be greater than 10",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "a".repeat(100),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                revisions: 10,
                delay: Duration::from_millis(60001),
            }
        }
        .validate(), @r###"
        Err(
            "Tracker delay cannot be greater than 60000ms",
        )
        "###);

        Ok(())
    }
}
