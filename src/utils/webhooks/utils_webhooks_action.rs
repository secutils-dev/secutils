use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebhooksAction {
    #[serde(rename_all = "camelCase")]
    GetAutoRespondersRequests { auto_responder_name: String },
}

#[cfg(test)]
mod tests {
    use crate::utils::UtilsWebhooksAction;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UtilsWebhooksAction>(
                r###"
{
    "type": "getAutoRespondersRequests",
    "value": { "autoResponderName": "some-name" }
}
          "###
            )?,
            UtilsWebhooksAction::GetAutoRespondersRequests {
                auto_responder_name: "some-name".to_string()
            }
        );

        Ok(())
    }
}
