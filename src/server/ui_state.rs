mod status;
mod status_level;
mod webhook_url_type;

pub use self::{status::Status, status_level::StatusLevel, webhook_url_type::WebhookUrlType};
use crate::{
    users::{ClientUserShare, SubscriptionFeatures, User, UserSettings},
    utils::Util,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiState<'a> {
    pub status: &'a Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<SubscriptionFeatures>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_share: Option<ClientUserShare>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<UserSettings>,
    pub utils: Vec<Util>,
    pub webhook_url_type: WebhookUrlType,
}

#[cfg(test)]
mod tests {
    use crate::{
        server::{Status, StatusLevel, UiState, WebhookUrlType},
        tests::{mock_config, mock_user},
        users::{ClientUserShare, SharedResource, UserId, UserShare, UserShareId},
        utils::Util,
    };
    use insta::assert_json_snapshot;
    use serde_json::json;
    use std::collections::BTreeMap;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let user = mock_user()?;
        let features = user.subscription.get_features(&mock_config()?);
        let ui_state = UiState {
            status: &Status {
                version: "1.0.0-alpha.4".to_string(),
                level: StatusLevel::Available,
            },
            user: Some(user),
            features: Some(features),
            user_share: Some(ClientUserShare::from(UserShare {
                id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000001")),
                user_id: UserId::default(),
                resource: SharedResource::content_security_policy(uuid!(
                    "00000000-0000-0000-0000-000000000001"
                )),
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            })),
            settings: Some(serde_json::from_value(serde_json::to_value(
                [("common.uiTheme".to_string(), Some(json!("light")))]
                    .into_iter()
                    .collect::<BTreeMap<_, _>>(),
            )?)?),
            utils: vec![Util {
                id: 1,
                handle: "some-handle".to_string(),
                name: "some-name".to_string(),
                keywords: Some("some keywords".to_string()),
                utils: None,
            }],
            webhook_url_type: WebhookUrlType::Path,
        };
        assert_json_snapshot!(ui_state, @r###"
        {
          "status": {
            "version": "1.0.0-alpha.4",
            "level": "available"
          },
          "user": {
            "email": "dev-1@secutils.dev",
            "handle": "dev-handle-1",
            "credentials": {
              "password": true,
              "passkey": false
            },
            "created": 1262340000,
            "activated": false,
            "subscription": {
              "tier": "ultimate",
              "startedAt": 1262340001
            }
          },
          "features": {
            "admin": true
          },
          "userShare": {
            "id": "00000000-0000-0000-0000-000000000001",
            "resource": {
              "type": "contentSecurityPolicy",
              "policyId": "00000000-0000-0000-0000-000000000001"
            },
            "createdAt": 946720800
          },
          "settings": {
            "common.uiTheme": "light"
          },
          "utils": [
            {
              "handle": "some-handle",
              "name": "some-name"
            }
          ],
          "webhookUrlType": "path"
        }
        "###);

        Ok(())
    }

    #[test]
    fn serialization_without_optional_properties() -> anyhow::Result<()> {
        let ui_state = UiState {
            status: &Status {
                version: "1.0.0-alpha.4".to_string(),
                level: StatusLevel::Available,
            },
            user: None,
            features: None,
            user_share: None,
            settings: None,
            utils: vec![],
            webhook_url_type: WebhookUrlType::Subdomain,
        };
        assert_json_snapshot!(ui_state, @r###"
        {
          "status": {
            "version": "1.0.0-alpha.4",
            "level": "available"
          },
          "utils": [],
          "webhookUrlType": "subdomain"
        }
        "###);

        Ok(())
    }
}
