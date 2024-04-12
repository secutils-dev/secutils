use crate::users::SubscriptionTier;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

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
#[serde(rename_all = "kebab-case")]
pub struct SecurityConfig {
    /// Name of the session cookie used by the authentication component.
    pub session_cookie_name: String,
    /// List of the preconfigured users, if specified.
    pub preconfigured_users: Option<HashMap<String, PreconfiguredUserConfig>>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            session_cookie_name: "id".to_string(),
            preconfigured_users: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        config::{security_config::PreconfiguredUserConfig, SecurityConfig},
        users::SubscriptionTier,
    };
    use insta::assert_toml_snapshot;

    #[test]
    fn serialization_and_default() {
        assert_toml_snapshot!(SecurityConfig::default(), @"session-cookie-name = 'id'");

        let config = SecurityConfig {
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
            ..Default::default()
        };

        assert_toml_snapshot!(config, @r###"
        session-cookie-name = 'id'
        [preconfigured-users."test@secutils.dev"]
        handle = 'test-handle'
        tier = 'basic'
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SecurityConfig = toml::from_str(
            r#"
        session-cookie-name = 'id'
    "#,
        )
        .unwrap();

        assert_eq!(
            config,
            SecurityConfig {
                session_cookie_name: "id".to_string(),
                preconfigured_users: None,
            }
        );

        let config: SecurityConfig = toml::from_str(
            r#"
        session-cookie-name = 'id'

        [preconfigured-users."test@secutils.dev"]
        handle = 'test-handle'
        tier = 'basic'
    "#,
        )
        .unwrap();

        assert_eq!(
            config,
            SecurityConfig {
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
                ..Default::default()
            }
        );
    }
}
