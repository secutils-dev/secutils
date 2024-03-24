mod status;
mod status_level;
mod webhook_url_type;

mod subscription_state;

pub use self::{
    status::Status, status_level::StatusLevel, subscription_state::SubscriptionState,
    webhook_url_type::WebhookUrlType,
};
use crate::{
    users::{ClientUserShare, User, UserSettings},
    utils::Util,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiState<'a, 'c> {
    pub status: &'a Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    #[serde(skip_serializing_if = "default")]
    pub subscription: SubscriptionState<'a, 'c>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_share: Option<ClientUserShare>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<UserSettings>,
    pub utils: Vec<Util>,
    pub webhook_url_type: WebhookUrlType,
}

fn default<T: Default + PartialEq>(t: &T) -> bool {
    *t == Default::default()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use insta::assert_json_snapshot;
    use serde_json::json;
    use time::OffsetDateTime;
    use url::Url;
    use uuid::uuid;

    use crate::{
        server::{
            ui_state::subscription_state::SubscriptionState, Status, StatusLevel, UiState,
            WebhookUrlType,
        },
        tests::{mock_config, mock_user},
        users::{ClientUserShare, SharedResource, UserId, UserShare, UserShareId},
        utils::Util,
    };

    #[test]
    fn serialization() -> anyhow::Result<()> {
        let user = mock_user()?;
        let config = mock_config()?;
        let features = user.subscription.get_features(&config);
        let manage_url = Url::parse("http://localhost:1234/subscription")?;
        let feature_overview_url = Url::parse("http://localhost:1234/features")?;
        let ui_state = UiState {
            status: &Status {
                version: "1.0.0-alpha.4".to_string(),
                level: StatusLevel::Available,
            },
            user: Some(user),
            subscription: SubscriptionState {
                features: Some(features),
                manage_url: Some(&manage_url),
                feature_overview_url: Some(&feature_overview_url),
            },
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
          "subscription": {
            "features": {
              "admin": true
            },
            "manageUrl": "http://localhost:1234/subscription",
            "featureOverviewUrl": "http://localhost:1234/features"
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
            subscription: Default::default(),
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
