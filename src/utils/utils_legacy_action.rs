use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::User,
    utils::{UtilsLegacyActionResult, UtilsWebhooksAction},
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsLegacyAction {
    Webhooks(UtilsWebhooksAction),
}

impl UtilsLegacyAction {
    /// Validates action parameters and throws if action parameters aren't valid.
    pub async fn validate(&self) -> anyhow::Result<()> {
        match self {
            UtilsLegacyAction::Webhooks(action) => action.validate(),
        }
    }

    /// Consumes and handles action.
    pub async fn handle<DR: DnsResolver, ET: EmailTransport>(
        self,
        user: User,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<UtilsLegacyActionResult> {
        match self {
            UtilsLegacyAction::Webhooks(action) => action
                .handle(user, api)
                .await
                .map(UtilsLegacyActionResult::Webhooks),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        AutoResponder, AutoResponderMethod, UtilsLegacyAction, UtilsWebhooksAction,
    };
    use insta::assert_debug_snapshot;

    #[tokio::test]
    async fn validation_webhooks() -> anyhow::Result<()> {
        assert!(
            UtilsLegacyAction::Webhooks(UtilsWebhooksAction::SaveAutoResponder {
                responder: AutoResponder {
                    path: "/name".to_string(),
                    method: AutoResponderMethod::Post,
                    requests_to_track: 3,
                    status_code: 200,
                    body: None,
                    headers: Some(vec![("key".to_string(), "value".to_string())]),
                    delay: None,
                }
            })
            .validate()
            .await
            .is_ok()
        );

        assert_debug_snapshot!(UtilsLegacyAction::Webhooks(UtilsWebhooksAction::SaveAutoResponder {
            responder: AutoResponder {
                path: "/name".to_string(),
                method: AutoResponderMethod::Post,
                requests_to_track: 3,
                status_code: 2000,
                body: None,
                headers: Some(vec![("key".to_string(), "value".to_string())]),
                delay: None,
            }
        })
        .validate().await, @r###"
        Err(
            "Auto responder is not valid.",
        )
        "###);

        assert!(
            UtilsLegacyAction::Webhooks(UtilsWebhooksAction::RemoveAutoResponder {
                responder_path: "/a".repeat(50),
            })
            .validate()
            .await
            .is_ok()
        );

        assert_debug_snapshot!(UtilsLegacyAction::Webhooks(UtilsWebhooksAction::RemoveAutoResponder {
            responder_path: "a".to_string(),
        })
        .validate().await, @r###"
        Err(
            "Auto responder path is not valid.",
        )
        "###);

        assert!(
            UtilsLegacyAction::Webhooks(UtilsWebhooksAction::GetAutoRespondersRequests {
                responder_path: "/a".repeat(50),
            })
            .validate()
            .await
            .is_ok()
        );

        assert_debug_snapshot!(UtilsLegacyAction::Webhooks(UtilsWebhooksAction::GetAutoRespondersRequests {
            responder_path: "a".to_string(),
        })
        .validate().await, @r###"
        Err(
            "Auto responder path is not valid.",
        )
        "###);

        Ok(())
    }
}
