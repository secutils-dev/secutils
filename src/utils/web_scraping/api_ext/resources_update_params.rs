use crate::utils::WebPageTrackerSettings;
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
    pub settings: Option<WebPageTrackerSettings>,
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        ResourcesUpdateParams, WebPageTrackerSettings,
        WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME,
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
                settings: Some(WebPageTrackerSettings {
                    revisions: 3,
                    schedule: Some("0 0 * * *".to_string()),
                    delay: Duration::from_millis(2000),
                    scripts: Some(
                        [(
                            WEB_PAGE_RESOURCES_TRACKER_FILTER_SCRIPT_NAME.to_string(),
                            "return resource;".to_string()
                        )]
                        .into_iter()
                        .collect()
                    ),
                    enable_notifications: true,
                }),
            }
        );

        Ok(())
    }
}
