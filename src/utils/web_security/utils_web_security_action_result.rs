use crate::{
    users::ClientUserShare,
    utils::{ContentSecurityPolicy, ContentSecurityPolicySource},
};
use serde::Serialize;

#[allow(clippy::enum_variant_names)]
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebSecurityActionResult {
    #[serde(rename_all = "camelCase")]
    GetContentSecurityPolicy {
        #[serde(skip_serializing_if = "Option::is_none")]
        policy: Option<ContentSecurityPolicy>,
        #[serde(skip_serializing_if = "Option::is_none")]
        user_share: Option<ClientUserShare>,
    },
    #[serde(rename_all = "camelCase")]
    SaveContentSecurityPolicy,
    #[serde(rename_all = "camelCase")]
    ImportContentSecurityPolicy,
    #[serde(rename_all = "camelCase")]
    RemoveContentSecurityPolicy,
    #[serde(rename_all = "camelCase")]
    SerializeContentSecurityPolicy {
        policy: String,
        source: ContentSecurityPolicySource,
    },
    #[serde(rename_all = "camelCase")]
    ShareContentSecurityPolicy { user_share: ClientUserShare },
    #[serde(rename_all = "camelCase")]
    UnshareContentSecurityPolicy {
        #[serde(skip_serializing_if = "Option::is_none")]
        user_share: Option<ClientUserShare>,
    },
}

impl UtilsWebSecurityActionResult {
    pub fn get(policy: Option<ContentSecurityPolicy>, user_share: Option<ClientUserShare>) -> Self {
        Self::GetContentSecurityPolicy { policy, user_share }
    }
    pub fn save() -> Self {
        Self::SaveContentSecurityPolicy
    }
    pub fn import() -> Self {
        Self::ImportContentSecurityPolicy
    }
    pub fn remove() -> Self {
        Self::RemoveContentSecurityPolicy
    }
    pub fn share(user_share: ClientUserShare) -> Self {
        Self::ShareContentSecurityPolicy { user_share }
    }
    pub fn unshare(user_share: Option<ClientUserShare>) -> Self {
        Self::UnshareContentSecurityPolicy { user_share }
    }

    pub fn serialize(serialized_policy: String, source: ContentSecurityPolicySource) -> Self {
        Self::SerializeContentSecurityPolicy {
            policy: serialized_policy,
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        users::{ClientUserShare, SharedResource, UserId, UserShare, UserShareId},
        utils::{
            ContentSecurityPolicy, ContentSecurityPolicyDirective, ContentSecurityPolicySource,
            UtilsWebSecurityActionResult,
        },
    };
    use insta::assert_json_snapshot;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(UtilsWebSecurityActionResult::get(
            Some(ContentSecurityPolicy {
                name: "policy-one".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string()].into_iter().collect(),
                )],
            }),
            Some(ClientUserShare::from(UserShare {
                id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000001")),
                user_id: UserId::empty(),
                resource: SharedResource::content_security_policy("policy-one".to_string()),
                created_at: time::OffsetDateTime::from_unix_timestamp(123456)?,
            }))
        ), @r###"
        {
          "type": "getContentSecurityPolicy",
          "value": {
            "policy": {
              "n": "policy-one",
              "d": [
                {
                  "n": "child-src",
                  "v": [
                    "'self'"
                  ]
                }
              ]
            },
            "userShare": {
              "id": "00000000-0000-0000-0000-000000000001",
              "resource": {
                "type": "contentSecurityPolicy",
                "policyName": "policy-one"
              },
              "createdAt": 123456
            }
          }
        }
        "###);

        assert_json_snapshot!(UtilsWebSecurityActionResult::save(), @r###"
        {
          "type": "saveContentSecurityPolicy"
        }
        "###);

        assert_json_snapshot!(UtilsWebSecurityActionResult::import(), @r###"
        {
          "type": "importContentSecurityPolicy"
        }
        "###);

        assert_json_snapshot!(UtilsWebSecurityActionResult::remove(), @r###"
        {
          "type": "removeContentSecurityPolicy"
        }
        "###);

        assert_json_snapshot!(UtilsWebSecurityActionResult::share(
          ClientUserShare::from(UserShare {
              id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000001")),
              user_id: UserId::empty(),
              resource: SharedResource::content_security_policy("policy-one".to_string()),
              created_at: time::OffsetDateTime::from_unix_timestamp(123456)?,
          })
        ), @r###"
        {
          "type": "shareContentSecurityPolicy",
          "value": {
            "userShare": {
              "id": "00000000-0000-0000-0000-000000000001",
              "resource": {
                "type": "contentSecurityPolicy",
                "policyName": "policy-one"
              },
              "createdAt": 123456
            }
          }
        }
        "###);

        assert_json_snapshot!(UtilsWebSecurityActionResult::unshare(
          Some(ClientUserShare::from(UserShare {
            id: UserShareId::from(uuid!("00000000-0000-0000-0000-000000000001")),
            user_id: UserId::empty(),
            resource: SharedResource::content_security_policy("policy-one".to_string()),
            created_at: time::OffsetDateTime::from_unix_timestamp(123456)?,
          }))
        ), @r###"
        {
          "type": "unshareContentSecurityPolicy",
          "value": {
            "userShare": {
              "id": "00000000-0000-0000-0000-000000000001",
              "resource": {
                "type": "contentSecurityPolicy",
                "policyName": "policy-one"
              },
              "createdAt": 123456
            }
          }
        }
        "###);

        assert_json_snapshot!(UtilsWebSecurityActionResult::unshare(
          None
        ), @r###"
        {
          "type": "unshareContentSecurityPolicy",
          "value": {}
        }
        "###);

        assert_json_snapshot!(UtilsWebSecurityActionResult::SerializeContentSecurityPolicy {
            policy: r###"default-src: 'self'; script-src: https:; report-to csp-prod-group"###.to_string(),
            source: ContentSecurityPolicySource::EnforcingHeader
        }, @r###"
        {
          "type": "serializeContentSecurityPolicy",
          "value": {
            "policy": "default-src: 'self'; script-src: https:; report-to csp-prod-group",
            "source": "enforcingHeader"
          }
        }
        "###);

        Ok(())
    }
}
