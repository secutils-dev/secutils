use crate::utils::WebPageResourcesTrackerSettings;
use serde::Deserialize;
use url::Url;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesCreateParams {
    /// Arbitrary name of the web page resources tracker.
    pub name: String,
    /// URL of the web page to track resources for.
    pub url: Url,
    /// Settings of the web page resources tracker.
    pub settings: WebPageResourcesTrackerSettings,
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        ResourcesCreateParams, WebPageResourcesTrackerScripts, WebPageResourcesTrackerSettings,
    };
    use std::time::Duration;
    use url::Url;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ResourcesCreateParams>(
                r#"
{
    "name": "pk",
    "url": "https://secutils.dev",
    "settings": {
        "revisions": 3,
        "delay": 2000,
        "enableNotifications": true
    }
}
          "#
            )?,
            ResourcesCreateParams {
                name: "pk".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: WebPageResourcesTrackerSettings {
                    revisions: 3,
                    schedule: None,
                    delay: Duration::from_millis(2000),
                    scripts: Default::default(),
                    enable_notifications: true,
                },
            }
        );

        assert_eq!(
            serde_json::from_str::<ResourcesCreateParams>(
                r#"
{
    "name": "pk",
    "url": "https://secutils.dev",
    "settings": {
        "revisions": 3,
        "delay": 2000,
        "schedule": "0 0 * * *",
        "scripts": {
            "resourceFilterMap": "return resource;"
        },
        "enableNotifications": true
    }
}
          "#
            )?,
            ResourcesCreateParams {
                name: "pk".to_string(),
                url: Url::parse("https://secutils.dev")?,
                settings: WebPageResourcesTrackerSettings {
                    revisions: 3,
                    schedule: Some("0 0 * * *".to_string()),
                    delay: Duration::from_millis(2000),
                    scripts: WebPageResourcesTrackerScripts {
                        resource_filter_map: Some("return resource;".to_string()),
                    },
                    enable_notifications: true,
                },
            }
        );

        Ok(())
    }
}
