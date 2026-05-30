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
    /// Maximum per-request timeout for `op_proxy_request` in milliseconds. Defaults to 30 seconds.
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    #[serde(default = "default_max_proxy_request_timeout")]
    pub max_proxy_request_timeout: Duration,
    /// Maximum length (in bytes) of a `secutils.kv.*` key. Defaults to 256 bytes.
    #[serde(default = "default_responder_kv_max_key_bytes")]
    pub responder_kv_max_key_bytes: usize,
    /// Maximum size (in bytes) of a single `secutils.kv.*` value. Defaults to 1 MiB.
    #[serde(default = "default_responder_kv_max_value_bytes")]
    pub responder_kv_max_value_bytes: usize,
    /// Maximum number of live entries a single responder may keep in its KV store. Defaults to 100000.
    #[serde(default = "default_responder_kv_max_entries")]
    pub responder_kv_max_entries: usize,
    /// Maximum total size (in bytes) of all live KV values for a single responder. Defaults to 1 GiB.
    #[serde(default = "default_responder_kv_max_total_bytes")]
    pub responder_kv_max_total_bytes: usize,
    /// Maximum TTL (in seconds) a `secutils.kv.set` may request. `0` means no TTL is allowed.
    /// Defaults to 30 days.
    #[serde(default = "default_responder_kv_max_ttl_sec")]
    pub responder_kv_max_ttl_sec: u64,
    /// Absolute ceiling (in seconds) on the lifetime of any `secutils.kv.*` row. Every write is
    /// capped to `now + this`, and TTL-less writes are forced to expire at that bound, so no row can
    /// outlive it - the backstop that makes the KV store self-cleaning even for abusive callers.
    /// `0` disables the cap (TTL-less writes become eternal again). Disabled by default.
    #[serde(default = "default_responder_kv_max_lifespan_sec")]
    pub responder_kv_max_lifespan_sec: u64,
    /// Maximum number of `secutils.kv.*` operations a single script invocation may perform.
    /// Defaults to 200.
    #[serde(default = "default_responder_kv_ops_per_script")]
    pub responder_kv_ops_per_script: usize,
}

fn default_responder_kv_max_key_bytes() -> usize {
    256
}

fn default_responder_kv_max_value_bytes() -> usize {
    1_048_576
}

fn default_responder_kv_max_entries() -> usize {
    100_000
}

fn default_responder_kv_max_total_bytes() -> usize {
    1_073_741_824
}

fn default_responder_kv_max_ttl_sec() -> u64 {
    30 * 24 * 3600
}

fn default_responder_kv_max_lifespan_sec() -> u64 {
    0
}

fn default_responder_kv_ops_per_script() -> usize {
    200
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

fn default_max_proxy_request_timeout() -> Duration {
    Duration::from_secs(30)
}

impl Default for SubscriptionWebhooksConfig {
    fn default() -> Self {
        Self {
            responders: 100,
            responder_requests: 30,
            js_runtime_heap_size: 10_485_760,
            js_runtime_script_execution_time: Duration::from_secs(30),
            restrict_to_public_urls: default_restrict_to_public_urls(),
            max_proxy_response_size: default_max_proxy_response_size(),
            max_concurrent_responder_requests: default_max_concurrent_responder_requests(),
            max_tracked_response_size: default_max_tracked_response_size(),
            max_proxy_request_timeout: default_max_proxy_request_timeout(),
            responder_kv_max_key_bytes: default_responder_kv_max_key_bytes(),
            responder_kv_max_value_bytes: default_responder_kv_max_value_bytes(),
            responder_kv_max_entries: default_responder_kv_max_entries(),
            responder_kv_max_total_bytes: default_responder_kv_max_total_bytes(),
            responder_kv_max_ttl_sec: default_responder_kv_max_ttl_sec(),
            responder_kv_max_lifespan_sec: default_responder_kv_max_lifespan_sec(),
            responder_kv_ops_per_script: default_responder_kv_ops_per_script(),
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
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000
        restrict_to_public_urls = true
        max_proxy_response_size = 10485760
        max_concurrent_responder_requests = 10
        max_tracked_response_size = 1048576
        max_proxy_request_timeout = 30000
        responder_kv_max_key_bytes = 256
        responder_kv_max_value_bytes = 1048576
        responder_kv_max_entries = 100000
        responder_kv_max_total_bytes = 1073741824
        responder_kv_max_ttl_sec = 2592000
        responder_kv_max_lifespan_sec = 0
        responder_kv_ops_per_script = 200
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SubscriptionWebhooksConfig = toml::from_str(
            r#"
        responders = 100
        responder_requests = 30
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000
        restrict_to_public_urls = true
        max_proxy_response_size = 10485760
        max_concurrent_responder_requests = 10
        max_tracked_response_size = 1048576
        max_proxy_request_timeout = 30000
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
        js_runtime_heap_size = 10485760
        js_runtime_script_execution_time = 30000
    "#,
        )
        .unwrap();
        assert_eq!(config, SubscriptionWebhooksConfig::default());
    }
}
