use serde::Deserialize;

#[derive(Deserialize, Default, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PageTrackerGetHistoryParams {
    #[serde(default)]
    pub refresh: bool,
    #[serde(default)]
    pub calculate_diff: bool,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_scraping::api_ext::PageTrackerGetHistoryParams;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<PageTrackerGetHistoryParams>(r#"{}"#)?,
            PageTrackerGetHistoryParams {
                refresh: false,
                calculate_diff: false
            }
        );

        assert_eq!(
            serde_json::from_str::<PageTrackerGetHistoryParams>(
                r#"
{
    "refresh": true,
    "calculateDiff": true
}
          "#
            )?,
            PageTrackerGetHistoryParams {
                refresh: true,
                calculate_diff: true
            }
        );

        Ok(())
    }
}
