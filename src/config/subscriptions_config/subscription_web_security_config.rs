use serde_derive::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct SubscriptionWebSecurityConfig {
    /// The number of policies (CSP, CORS etc.) available to a particular subscription.
    pub policies: usize,
    /// Indicates whether it's allowed to import policies from a URL for a particular subscription.
    pub import_policy_from_url: bool,
}

impl Default for SubscriptionWebSecurityConfig {
    fn default() -> Self {
        Self {
            policies: 1000,
            import_policy_from_url: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::SubscriptionWebSecurityConfig;
    use insta::assert_toml_snapshot;

    #[test]
    fn serialization_and_default() {
        assert_toml_snapshot!(SubscriptionWebSecurityConfig::default(), @r###"
        policies = 1000
        import-policy-from-url = true
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SubscriptionWebSecurityConfig = toml::from_str(
            r#"
        policies = 100
        import-policy-from-url = true
    "#,
        )
        .unwrap();
        assert_eq!(
            config,
            SubscriptionWebSecurityConfig {
                policies: 100,
                import_policy_from_url: true,
            }
        );
    }
}
