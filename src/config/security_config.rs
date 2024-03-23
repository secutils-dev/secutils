use crate::users::SubscriptionTier;
use hex::ToHex;
use rand_core::{OsRng, RngCore};
use serde_derive::{Deserialize, Serialize};

pub const SESSION_KEY_LENGTH_BYTES: usize = 64;

/// Describes the builtin user configuration.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct BuiltinUserConfig {
    /// Builtin user email.
    pub email: String,
    /// Builtin user handle (used to construct unique user sub-domain).
    pub handle: String,
    /// Builtin user credentials.
    pub password: String,
    /// Builtin user subscription tier.
    pub tier: SubscriptionTier,
}

/// Configuration for the SMTP functionality.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct SecurityConfig {
    /// Session encryption key.
    pub session_key: String,
    /// Indicates that server shouldn't set `Secure` flag on the session cookie (do not use in production).
    pub use_insecure_session_cookie: bool,
    /// List of the builtin users, if specified.
    pub builtin_users: Option<Vec<BuiltinUserConfig>>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        let mut session_key = [0; SESSION_KEY_LENGTH_BYTES / 2];
        OsRng.fill_bytes(&mut session_key);

        Self {
            session_key: session_key.encode_hex(),
            use_insecure_session_cookie: false,
            builtin_users: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        config::{BuiltinUserConfig, SecurityConfig, SESSION_KEY_LENGTH_BYTES},
        users::SubscriptionTier,
    };
    use insta::assert_toml_snapshot;

    #[test]
    fn serialization_and_default() {
        let mut default_config = SecurityConfig::default();
        assert_eq!(default_config.session_key.len(), SESSION_KEY_LENGTH_BYTES);
        assert!(default_config.builtin_users.is_none());
        assert!(!default_config.use_insecure_session_cookie);

        default_config.session_key = "a".repeat(SESSION_KEY_LENGTH_BYTES);

        assert_toml_snapshot!(default_config, @r###"
        session-key = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
        use-insecure-session-cookie = false
        "###);

        default_config.builtin_users = Some(vec![BuiltinUserConfig {
            email: "test@secutils.dev".to_string(),
            handle: "test-handle".to_string(),
            password: "test-password".to_string(),
            tier: SubscriptionTier::Basic,
        }]);

        assert_toml_snapshot!(default_config, @r###"
        session-key = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
        use-insecure-session-cookie = false

        [[builtin-users]]
        email = 'test@secutils.dev'
        handle = 'test-handle'
        password = 'test-password'
        tier = 'basic'
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SecurityConfig = toml::from_str(
            r#"
        session-key = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
        use-insecure-session-cookie = true

        [[builtin-users]]
        email = 'test@secutils.dev'
        handle = 'test-handle'
        password = 'test-password'
        tier = 'basic'
    "#,
        )
        .unwrap();

        assert_eq!(
            config,
            SecurityConfig {
                session_key: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
                use_insecure_session_cookie: true,
                builtin_users: Some(vec![BuiltinUserConfig {
                    email: "test@secutils.dev".to_string(),
                    handle: "test-handle".to_string(),
                    password: "test-password".to_string(),
                    tier: SubscriptionTier::Basic,
                }])
            }
        );
    }
}
