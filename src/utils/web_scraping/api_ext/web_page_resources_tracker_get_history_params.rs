use serde::Deserialize;

#[derive(Deserialize, Default, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebPageResourcesTrackerGetHistoryParams {
    #[serde(default)]
    pub refresh: bool,
    #[serde(default)]
    pub calculate_diff: bool,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_scraping::api_ext::WebPageResourcesTrackerGetHistoryParams;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebPageResourcesTrackerGetHistoryParams>(r#"{}"#)?,
            WebPageResourcesTrackerGetHistoryParams {
                refresh: false,
                calculate_diff: false,
            }
        );

        assert_eq!(
            serde_json::from_str::<WebPageResourcesTrackerGetHistoryParams>(
                r#"
{
    "refresh": true,
    "calculateDiff": true
}
          "#
            )?,
            WebPageResourcesTrackerGetHistoryParams {
                refresh: true,
                calculate_diff: true,
            }
        );

        Ok(())
    }
}
