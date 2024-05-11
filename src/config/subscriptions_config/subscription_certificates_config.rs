use serde_derive::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionCertificatesConfig {
    /// The number of private keys available to a particular subscription.
    pub private_keys: usize,
    /// The number of certificate templates for a particular subscription.
    pub templates: usize,
    /// The list of allowed private key algorithms for a particular subscription.
    pub private_key_algorithms: Option<HashSet<String>>,
}

impl Default for SubscriptionCertificatesConfig {
    fn default() -> Self {
        Self {
            private_keys: 100,
            templates: 1000,
            // Default to None to allow all private key algorithms.
            private_key_algorithms: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        config::SubscriptionCertificatesConfig,
        utils::certificates::{PrivateKeyAlgorithm, PrivateKeySize},
    };
    use insta::assert_toml_snapshot;

    #[test]
    fn serialization_and_default() {
        let config = SubscriptionCertificatesConfig::default();
        assert_toml_snapshot!(config, @r###"
        private_keys = 100
        templates = 1000
        "###);

        let config = SubscriptionCertificatesConfig {
            private_keys: 1,
            templates: 11,
            private_key_algorithms: Some(
                [PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size1024,
                }
                .to_string()]
                .into_iter()
                .collect(),
            ),
        };
        assert_toml_snapshot!(config, @r###"
        private_keys = 1
        templates = 11
        private_key_algorithms = ['RSA-1024']
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SubscriptionCertificatesConfig = toml::from_str(
            r#"
        private_keys = 1
        templates = 11
        private_key_algorithms = ['RSA-1024']
    "#,
        )
        .unwrap();
        assert_eq!(
            config,
            SubscriptionCertificatesConfig {
                private_keys: 1,
                templates: 11,
                private_key_algorithms: Some(
                    [PrivateKeyAlgorithm::Rsa {
                        key_size: PrivateKeySize::Size1024,
                    }
                    .to_string()]
                    .into_iter()
                    .collect(),
                ),
            }
        );
    }
}
