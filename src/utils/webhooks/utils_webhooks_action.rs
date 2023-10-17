use crate::{
    api::Api,
    error::Error as SecutilsError,
    network::{DnsResolver, EmailTransport},
    users::User,
    utils::{AutoResponder, UtilsWebhooksActionResult},
};
use anyhow::bail;
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebhooksAction {
    #[serde(rename_all = "camelCase")]
    SaveAutoResponder { responder: AutoResponder },
    #[serde(rename_all = "camelCase")]
    RemoveAutoResponder { responder_path: String },
    #[serde(rename_all = "camelCase")]
    GetAutoRespondersRequests { responder_path: String },
}

impl UtilsWebhooksAction {
    /// Validates action parameters and throws if action parameters aren't valid.
    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            UtilsWebhooksAction::GetAutoRespondersRequests { responder_path }
            | UtilsWebhooksAction::RemoveAutoResponder { responder_path } => {
                if !AutoResponder::is_path_valid(responder_path) {
                    bail!(SecutilsError::client("Auto responder path is not valid."));
                }
            }
            UtilsWebhooksAction::SaveAutoResponder { responder } => {
                if !responder.is_valid() {
                    bail!(SecutilsError::client("Auto responder is not valid."));
                }
            }
        }

        Ok(())
    }

    pub async fn handle<DR: DnsResolver, ET: EmailTransport>(
        self,
        user: User,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<UtilsWebhooksActionResult> {
        let auto_responders = api.auto_responders();
        match self {
            UtilsWebhooksAction::SaveAutoResponder { responder } => auto_responders
                .upsert_auto_responder(user.id, responder)
                .await
                .map(|_| UtilsWebhooksActionResult::save()),
            UtilsWebhooksAction::RemoveAutoResponder { responder_path } => auto_responders
                .remove_auto_responder(user.id, &responder_path)
                .await
                .map(|_| UtilsWebhooksActionResult::remove()),
            UtilsWebhooksAction::GetAutoRespondersRequests { responder_path } => {
                Ok(UtilsWebhooksActionResult::get_requests(
                    if let Some(auto_responder) = auto_responders
                        .get_auto_responder(user.id, &responder_path)
                        .await?
                    {
                        auto_responders
                            .get_requests(user.id, &auto_responder)
                            .await?
                    } else {
                        Vec::with_capacity(0)
                    },
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{AutoResponder, AutoResponderMethod, UtilsWebhooksAction};
    use insta::assert_debug_snapshot;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UtilsWebhooksAction>(
                r#"
{
    "type": "getAutoRespondersRequests",
    "value": { "responderPath": "/some-name" }
}
          "#
            )?,
            UtilsWebhooksAction::GetAutoRespondersRequests {
                responder_path: "/some-name".to_string()
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebhooksAction>(
                r#"
{
    "type": "removeAutoResponder",
    "value": { "responderPath": "/some-name" }
}
          "#
            )?,
            UtilsWebhooksAction::RemoveAutoResponder {
                responder_path: "/some-name".to_string()
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebhooksAction>(
                r#"
{
    "type": "saveAutoResponder",
    "value": { "responder": {
          "p": "/name",
          "m": "p",
          "t": 3,
          "s": 200,
          "h": [["key", "value"]]
        } }
}
          "#
            )?,
            UtilsWebhooksAction::SaveAutoResponder {
                responder: AutoResponder {
                    path: "/name".to_string(),
                    method: AutoResponderMethod::Post,
                    requests_to_track: 3,
                    status_code: 200,
                    body: None,
                    headers: Some(vec![("key".to_string(), "value".to_string())]),
                    delay: None,
                }
            }
        );

        Ok(())
    }

    #[test]
    fn validation() -> anyhow::Result<()> {
        assert!(UtilsWebhooksAction::GetAutoRespondersRequests {
            responder_path: "/a".repeat(50),
        }
        .validate()
        .is_ok());

        assert_debug_snapshot!(UtilsWebhooksAction::GetAutoRespondersRequests {
            responder_path: "".to_string(),
        }
        .validate(), @r###"
        Err(
            "Auto responder path is not valid.",
        )
        "###);

        assert_debug_snapshot!(UtilsWebhooksAction::GetAutoRespondersRequests {
            responder_path: "/a".repeat(51),
        }
        .validate(), @r###"
        Err(
            "Auto responder path is not valid.",
        )
        "###);

        assert!(UtilsWebhooksAction::RemoveAutoResponder {
            responder_path: "/a".repeat(50),
        }
        .validate()
        .is_ok());

        assert_debug_snapshot!(UtilsWebhooksAction::RemoveAutoResponder{
            responder_path: "".to_string(),
        }
        .validate(), @r###"
        Err(
            "Auto responder path is not valid.",
        )
        "###);

        assert_debug_snapshot!(UtilsWebhooksAction::RemoveAutoResponder {
            responder_path: "/a".repeat(51),
        }
        .validate(), @r###"
        Err(
            "Auto responder path is not valid.",
        )
        "###);

        assert!(UtilsWebhooksAction::SaveAutoResponder {
            responder: AutoResponder {
                path: "/name".to_string(),
                method: AutoResponderMethod::Post,
                requests_to_track: 3,
                status_code: 200,
                body: None,
                headers: Some(vec![("key".to_string(), "value".to_string())]),
                delay: None,
            }
        }
        .validate()
        .is_ok());

        assert_debug_snapshot!(UtilsWebhooksAction::SaveAutoResponder {
            responder: AutoResponder {
                path: "/name".to_string(),
                method: AutoResponderMethod::Post,
                requests_to_track: 3,
                status_code: 1200,
                body: None,
                headers: Some(vec![("key".to_string(), "value".to_string())]),
                delay: None,
            }
        }
        .validate(), @r###"
        Err(
            "Auto responder is not valid.",
        )
        "###);

        assert_debug_snapshot!(UtilsWebhooksAction::SaveAutoResponder {
            responder: AutoResponder {
                path: "/a".repeat(51),
                method: AutoResponderMethod::Post,
                requests_to_track: 3,
                status_code: 200,
                body: None,
                headers: Some(vec![("key".to_string(), "value".to_string())]),
                delay: None,
            }
        }
        .validate(), @r###"
        Err(
            "Auto responder is not valid.",
        )
        "###);

        Ok(())
    }
}
