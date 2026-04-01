use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Deserialize, Default, Debug, Clone, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"refresh": false}))]
pub struct ApiTrackerGetHistoryParams {
    #[serde(default)]
    pub refresh: bool,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_scraping::api_ext::ApiTrackerGetHistoryParams;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ApiTrackerGetHistoryParams>(r#"{}"#)?,
            ApiTrackerGetHistoryParams { refresh: false }
        );

        assert_eq!(
            serde_json::from_str::<ApiTrackerGetHistoryParams>(r#"{ "refresh": true }"#)?,
            ApiTrackerGetHistoryParams { refresh: true }
        );

        Ok(())
    }
}
