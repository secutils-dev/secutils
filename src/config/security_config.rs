use crate::users::SubscriptionTier;
use serde_derive::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Describes the preconfigured user configuration.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct PreconfiguredUserConfig {
    /// Preconfigured user handle (used to construct unique user sub-domain).
    pub handle: String,
    /// Preconfigured user subscription tier.
    pub tier: SubscriptionTier,
}

/// Configuration for the SMTP functionality.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct SecurityConfig {
    /// Name of the session cookie used by the authentication component.
    pub session_cookie_name: String,
    /// Secret key used to sign JWT tokens used for HTTP authentication. If not provided, HTTP
    /// authentication will be disabled.
    pub jwt_secret: Option<String>,
    /// List of user or service account identifiers that should be treated as operators, if specified.
    pub operators: Option<HashSet<String>>,
    /// List of the preconfigured users, if specified.
    pub preconfigured_users: Option<HashMap<String, PreconfiguredUserConfig>>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            session_cookie_name: "id".to_string(),
            jwt_secret: None,
            preconfigured_users: None,
            operators: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        config::{SecurityConfig, security_config::PreconfiguredUserConfig},
        users::SubscriptionTier,
    };
    use insta::assert_toml_snapshot;

    #[test]
    fn serialization_and_default() {
        assert_toml_snapshot!(SecurityConfig::default(), @"session_cookie_name = 'id'");

        let config = SecurityConfig {
            session_cookie_name: "id".to_string(),
            jwt_secret: Some("3024bf8975b03b84e405f36a7bacd1c1".to_string()),
            operators: Some(["test@secutils.dev".to_string()].into_iter().collect()),
            preconfigured_users: Some(
                [(
                    "test@secutils.dev".to_string(),
                    PreconfiguredUserConfig {
                        handle: "test-handle".to_string(),
                        tier: SubscriptionTier::Basic,
                    },
                )]
                .into_iter()
                .collect(),
            ),
        };

        assert_toml_snapshot!(config, @r###"
        session_cookie_name = 'id'
        jwt_secret = '3024bf8975b03b84e405f36a7bacd1c1'
        operators = ['test@secutils.dev']
        [preconfigured_users."test@secutils.dev"]
        handle = 'test-handle'
        tier = 'basic'
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SecurityConfig = toml::from_str(
            r#"
        session_cookie_name = 'id'
    "#,
        )
        .unwrap();

        assert_eq!(
            config,
            SecurityConfig {
                session_cookie_name: "id".to_string(),
                jwt_secret: None,
                preconfigured_users: None,
                operators: None,
            }
        );

        let config: SecurityConfig = toml::from_str(
            r#"
        session_cookie_name = 'id'
        jwt_secret = '3024bf8975b03b84e405f36a7bacd1c1'
        operators = ['test@secutils.dev']

        [preconfigured_users."test@secutils.dev"]
        handle = 'test-handle'
        tier = 'basic'
    "#,
        )
        .unwrap();

        assert_eq!(
            config,
            SecurityConfig {
                jwt_secret: Some("3024bf8975b03b84e405f36a7bacd1c1".to_string()),
                preconfigured_users: Some(
                    [(
                        "test@secutils.dev".to_string(),
                        PreconfiguredUserConfig {
                            handle: "test-handle".to_string(),
                            tier: SubscriptionTier::Basic,
                        },
                    )]
                    .into_iter()
                    .collect(),
                ),
                operators: Some(["test@secutils.dev".to_string()].into_iter().collect()),
                ..Default::default()
            }
        );
    }
}
