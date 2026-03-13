use serde_derive::{Deserialize, Serialize};
use serde_with::{DurationMilliSeconds, serde_as};
use std::time::Duration;

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
pub struct SubscriptionWebhooksConfig {
    /// The number of responders available to a particular subscription.
    pub responders: usize,
    /// The number of responders requests per responder that retained for a particular subscription.
    pub responder_requests: usize,
    /// Indicates whether the subscription supports custom prefix for a responder subdomain.
    pub responder_custom_subdomain_prefix: bool,
    /// The hard limit for the JS runtime heap size in bytes. Defaults to 10485760 bytes or 10 MB.
    pub js_runtime_heap_size: usize,
    /// The maximum duration for a single JS script execution. Defaults to 30 seconds.
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub js_runtime_script_execution_time: Duration,
    /// Whether `op_proxy_request` restricts target URLs to public addresses (SSRF prevention).
    #[serde(default = "default_restrict_to_public_urls")]
    pub restrict_to_public_urls: bool,
    /// Maximum upstream response body size (in bytes) for `op_proxy_request`.
    #[serde(default = "default_max_proxy_response_size")]
    pub max_proxy_response_size: usize,
    /// Maximum number of concurrent requests a single responder can handle simultaneously.
    #[serde(default = "default_max_concurrent_responder_requests")]
    pub max_concurrent_responder_requests: usize,
    /// Maximum response body size (in bytes) stored when response tracking is enabled.
    #[serde(default = "default_max_tracked_response_size")]
    pub max_tracked_response_size: usize,
}

fn default_restrict_to_public_urls() -> bool {
    true
}

fn default_max_proxy_response_size() -> usize {
    10_485_760
}

fn default_max_concurrent_responder_requests() -> usize {
    10
}

fn default_max_tracked_response_size() -> usize {
    1_048_576
}

impl Default for SubscriptionWebhooksConfig {
    fn default() -> Self {
        Self {
            responders: 100,
            responder_requests: 30,
            responder_custom_subdomain_prefix: true,
            js_runtime_heap_size: 10_485_760,
            js_runtime_script_execution_time: Duration::from_secs(30),
            restrict_to_public_urls: default_restrict_to_public_urls(),
            max_proxy_response_size: default_max_proxy_response_size(),
            max_concurrent_responder_requests: default_max_concurrent_responder_requests(),
            max_tracked_response_size: default_max_tracked_response_size(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::SubscriptionWebhooksConfig;
    use insta::assert_toml_snapshot;

    #[test]
    fn serialization_and_default() {
        let config = SubscriptionWebhooksConfig::default();
        assert_toml_snapshot!(config, @r###"
        responders = 100
        responder_requests = 30
        responder_custom_subdomain_prefix = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000
        restrict_to_public_urls = true
        max_proxy_response_size = 10485760
        max_concurrent_responder_requests = 10
        max_tracked_response_size = 1048576
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SubscriptionWebhooksConfig = toml::from_str(
            r#"
        responders = 100
        responder_requests = 30
        responder_custom_subdomain_prefix = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000
        restrict_to_public_urls = true
        max_proxy_response_size = 10485760
        max_concurrent_responder_requests = 10
        max_tracked_response_size = 1048576
    "#,
        )
        .unwrap();
        assert_eq!(config, SubscriptionWebhooksConfig::default());
    }

    #[test]
    fn deserialization_with_defaults_for_new_fields() {
        let config: SubscriptionWebhooksConfig = toml::from_str(
            r#"
        responders = 100
        responder_requests = 30
        responder_custom_subdomain_prefix = true
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000
    "#,
        )
        .unwrap();
        assert_eq!(config, SubscriptionWebhooksConfig::default());
    }
}
