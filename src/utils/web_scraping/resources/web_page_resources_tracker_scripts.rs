use serde::{Deserialize, Serialize};

/// Scripts to inject into the web page before extracting resources to track.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct WebPageResourcesTrackerScripts {
    /// Optional script used to `filter_map` resource that needs to be tracked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_filter_map: Option<String>,
}

impl WebPageResourcesTrackerScripts {
    /// Returns `true` if none of the scripts are set.
    pub fn is_empty(&self) -> bool {
        self.resource_filter_map.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::WebPageResourcesTrackerScripts;
    use insta::assert_json_snapshot;
    use serde_json::json;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let scripts = WebPageResourcesTrackerScripts {
            resource_filter_map: None,
        };
        assert_json_snapshot!(scripts, @"{}");

        let scripts = WebPageResourcesTrackerScripts {
            resource_filter_map: Some("return resource;".to_string()),
        };
        assert_json_snapshot!(scripts, @r###"
        {
          "resourceFilterMap": "return resource;"
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        let scripts = WebPageResourcesTrackerScripts {
            resource_filter_map: None,
        };
        assert_eq!(
            serde_json::from_str::<WebPageResourcesTrackerScripts>(&json!({}).to_string())?,
            scripts
        );

        let scripts = WebPageResourcesTrackerScripts {
            resource_filter_map: Some("return resource;".to_string()),
        };
        assert_eq!(
            serde_json::from_str::<WebPageResourcesTrackerScripts>(
                &json!({ "resourceFilterMap": "return resource;" }).to_string()
            )?,
            scripts
        );

        Ok(())
    }

    #[test]
    fn is_empty() {
        let scripts = WebPageResourcesTrackerScripts {
            resource_filter_map: None,
        };
        assert!(scripts.is_empty());

        let scripts = WebPageResourcesTrackerScripts {
            resource_filter_map: Some("return resource.url !== undefined;".to_string()),
        };
        assert!(!scripts.is_empty());
    }
}
