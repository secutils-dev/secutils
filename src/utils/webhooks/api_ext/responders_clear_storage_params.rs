use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq, Default, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"prefix": "req/"}))]
pub struct RespondersClearStorageParams {
    /// When set, only entries whose key starts with this prefix are removed. When omitted, the
    /// entire storage is cleared.
    pub prefix: Option<String>,
}

#[cfg(test)]
mod tests {
    use crate::utils::webhooks::RespondersClearStorageParams;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<RespondersClearStorageParams>(r#"{ "prefix": "req/" }"#)?,
            RespondersClearStorageParams {
                prefix: Some("req/".to_string())
            }
        );
        assert_eq!(
            serde_json::from_str::<RespondersClearStorageParams>("{}")?,
            RespondersClearStorageParams::default()
        );

        Ok(())
    }
}
