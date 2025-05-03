use serde::{Deserialize, Serialize};
use url::Url;

/// Configuration for the integration with Retrack service.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct RetrackConfig {
    /// The URL to access the Retrack service.
    pub host: Url,
}

impl Default for RetrackConfig {
    fn default() -> Self {
        Self {
            host: Url::parse("http://localhost:7676")
                .expect("Cannot parse Retrack host parameter."),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::RetrackConfig;
    use insta::assert_debug_snapshot;

    #[test]
    fn default() {
        assert_debug_snapshot!(RetrackConfig::default(), @r###"
        RetrackConfig {
            host: Url {
                scheme: "http",
                cannot_be_a_base: false,
                username: "",
                password: None,
                host: Some(
                    Domain(
                        "localhost",
                    ),
                ),
                port: Some(
                    7676,
                ),
                path: "/",
                query: None,
                fragment: None,
            },
        }
        "###);
    }

    #[test]
    fn deserialization() {
        let config: RetrackConfig = toml::from_str(
            r#"
        host = 'http://localhost:8686/'
    "#,
        )
        .unwrap();
        assert_eq!(
            config,
            RetrackConfig {
                host: url::Url::parse("http://localhost:8686").unwrap(),
            }
        );
    }
}
