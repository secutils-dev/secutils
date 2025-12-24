use serde_derive::{Deserialize, Serialize};
use serde_with::{DurationMilliSeconds, serde_as};
use std::time::Duration;

/// Configuration for the HTTP functionality.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct HttpConfig {
    /// Configuration for the HTTP client.
    pub client: HttpClientConfig,
}

/// Describes the HTTP client configuration.
#[serde_as]
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct HttpClientConfig {
    /// Total request timeout. The timeout is applied from when the request starts connecting until
    /// the response body has finished. Also considered a total deadline. Default is 30 seconds.
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    #[serde(default = "default_timeout")]
    pub timeout: Duration,
    /// Timeout for idle sockets being kept-alive. Default is 5 seconds.
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    #[serde(default = "default_pool_idle_timeout")]
    pub pool_idle_timeout: Duration,
    /// Maximum number of retries (with exponential backoff) for HTTP requests if they fail because
    /// of transient errors. Setting this to 0 will disable retries. The default value is 3.
    pub max_retries: u32,
    /// Defines whether HTTP client connections should emit verbose logs. Default is false.
    pub verbose: bool,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout: default_timeout(),
            pool_idle_timeout: default_pool_idle_timeout(),
            max_retries: 3,
            verbose: false,
        }
    }
}

/// Defines default timeout for idle sockets being kept-alive.
const fn default_pool_idle_timeout() -> Duration {
    Duration::from_secs(5)
}

/// Default total request timeout.
const fn default_timeout() -> Duration {
    Duration::from_secs(30)
}

#[cfg(test)]
mod tests {
    use super::{HttpClientConfig, HttpConfig};
    use insta::assert_toml_snapshot;
    use std::time::Duration;

    #[test]
    fn serialization_and_default() {
        assert_toml_snapshot!(HttpConfig::default(), @"client = { timeout = 30000, pool_idle_timeout = 5000, max_retries = 3, verbose = false }");

        let config = HttpConfig {
            client: HttpClientConfig {
                timeout: Duration::from_secs(60),
                pool_idle_timeout: Duration::from_secs(10),
                max_retries: 5,
                verbose: true,
            },
        };

        assert_toml_snapshot!(config, @"client = { timeout = 60000, pool_idle_timeout = 10000, max_retries = 5, verbose = true }");
    }

    #[test]
    fn deserialization() {
        let config: HttpConfig = toml::from_str(
            r#"
        [client]
        timeout = 30000
        pool_idle_timeout = 5000
        max_retries = 3
        verbose = false
    "#,
        )
        .unwrap();
        assert_eq!(config, HttpConfig::default());

        let config: HttpConfig = toml::from_str(
            r#"
        [client]
        timeout = 60000
        pool_idle_timeout = 10000
        max_retries = 5
        verbose = true
    "#,
        )
        .unwrap();
        assert_eq!(
            config,
            HttpConfig {
                client: HttpClientConfig {
                    timeout: Duration::from_secs(60),
                    pool_idle_timeout: Duration::from_secs(10),
                    max_retries: 5,
                    verbose: true,
                },
            }
        );
    }
}
