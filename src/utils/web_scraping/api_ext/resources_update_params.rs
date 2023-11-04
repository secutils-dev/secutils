use crate::utils::WebPageResourcesTrackerSettings;
use serde::Deserialize;
use url::Url;

#[derive(Deserialize, Debug, Default, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesUpdateParams {
    /// Arbitrary name of the web page resources tracker.
    pub name: Option<String>,
    /// URL of the web page to track resources for.
    pub url: Option<Url>,
    /// Settings of the web page resources tracker.
    pub settings: Option<WebPageResourcesTrackerSettings>,
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        ResourcesUpdateParams, WebPageResourcesTrackerScripts, WebPageResourcesTrackerSettings,
    };
    use std::time::Duration;
    use url::Url;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ResourcesUpdateParams>(
                r#"
{
    "name": "pk"
}
          "#
            )?,
            ResourcesUpdateParams {
                name: Some("pk".to_string()),
                url: None,
                settings: None,
            }
        );

        assert_eq!(
            serde_json::from_str::<ResourcesUpdateParams>(
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
            ResourcesUpdateParams {
                name: Some("pk".to_string()),
                url: Some(Url::parse("https://secutils.dev")?),
                settings: Some(WebPageResourcesTrackerSettings {
                    revisions: 3,
                    schedule: Some("0 0 * * *".to_string()),
                    delay: Duration::from_millis(2000),
                    scripts: WebPageResourcesTrackerScripts {
                        resource_filter_map: Some("return resource;".to_string()),
                    },
                    enable_notifications: true,
                }),
            }
        );

        Ok(())
    }
}
