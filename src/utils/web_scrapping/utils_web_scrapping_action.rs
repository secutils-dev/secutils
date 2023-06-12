use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebScrappingAction {
    #[serde(rename_all = "camelCase")]
    TrackWebPageResources { tracker_name: String },
}

#[cfg(test)]
mod tests {
    use crate::utils::UtilsWebScrappingAction;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UtilsWebScrappingAction>(
                r###"
{
    "type": "trackWebPageResources",
    "value": { "trackerName": "tracker" }
}
          "###
            )?,
            UtilsWebScrappingAction::TrackWebPageResources {
                tracker_name: "tracker".to_string()
            }
        );

        Ok(())
    }
}
