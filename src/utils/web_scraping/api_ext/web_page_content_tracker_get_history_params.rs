use serde::Deserialize;

#[derive(Deserialize, Default, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebPageContentTrackerGetHistoryParams {
    #[serde(default)]
    pub refresh: bool,
}

#[cfg(test)]
mod tests {
    use crate::utils::WebPageContentTrackerGetHistoryParams;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<WebPageContentTrackerGetHistoryParams>(r#"{}"#)?,
            WebPageContentTrackerGetHistoryParams { refresh: false }
        );

        assert_eq!(
            serde_json::from_str::<WebPageContentTrackerGetHistoryParams>(
                r#"
{
    "refresh": true
}
          "#
            )?,
            WebPageContentTrackerGetHistoryParams { refresh: true }
        );

        Ok(())
    }
}
