use serde::Deserialize;

#[derive(Deserialize, Default, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesGetHistoryParams {
    #[serde(default)]
    pub refresh: bool,
    #[serde(default)]
    pub calculate_diff: bool,
}

#[cfg(test)]
mod tests {
    use crate::utils::ResourcesGetHistoryParams;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ResourcesGetHistoryParams>(r#"{}"#)?,
            ResourcesGetHistoryParams {
                refresh: false,
                calculate_diff: false,
            }
        );

        assert_eq!(
            serde_json::from_str::<ResourcesGetHistoryParams>(
                r#"
{
    "refresh": true,
    "calculateDiff": true
}
          "#
            )?,
            ResourcesGetHistoryParams {
                refresh: true,
                calculate_diff: true,
            }
        );

        Ok(())
    }
}
