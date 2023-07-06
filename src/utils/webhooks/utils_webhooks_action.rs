use crate::utils::utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebhooksAction {
    #[serde(rename_all = "camelCase")]
    GetAutoRespondersRequests { auto_responder_name: String },
}

impl UtilsWebhooksAction {
    /// Validates action parameters and throws if action parameters aren't valid.
    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            UtilsWebhooksAction::GetAutoRespondersRequests {
                auto_responder_name,
            } => {
                if auto_responder_name.is_empty() {
                    anyhow::bail!("Auto responder name cannot be empty");
                }

                if auto_responder_name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
                    anyhow::bail!(
                        "Auto responder name cannot be longer than {} characters",
                        MAX_UTILS_ENTITY_NAME_LENGTH
                    );
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::UtilsWebhooksAction;
    use insta::assert_debug_snapshot;

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

    #[test]
    fn validation() -> anyhow::Result<()> {
        assert!(UtilsWebhooksAction::GetAutoRespondersRequests {
            auto_responder_name: "a".repeat(100),
        }
        .validate()
        .is_ok());

        assert_debug_snapshot!(UtilsWebhooksAction::GetAutoRespondersRequests {
            auto_responder_name: "".to_string(),
        }
        .validate(), @r###"
        Err(
            "Auto responder name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsWebhooksAction::GetAutoRespondersRequests {
            auto_responder_name: "a".repeat(101),
        }
        .validate(), @r###"
        Err(
            "Auto responder name cannot be longer than 100 characters",
        )
        "###);

        Ok(())
    }
}
