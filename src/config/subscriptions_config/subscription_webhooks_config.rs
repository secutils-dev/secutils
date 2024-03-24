use serde_derive::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};
use std::time::Duration;

#[serde_as]
#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
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
}

impl Default for SubscriptionWebhooksConfig {
    fn default() -> Self {
        Self {
            responders: 100,
            responder_requests: 100,
            js_runtime_heap_size: 10_485_760,
            js_runtime_script_execution_time: Duration::from_secs(30),
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
        responder-requests = 100
        js-runtime-heap-size = 10485760
        js-runtime-script-execution-time = 30000
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SubscriptionWebhooksConfig = toml::from_str(
            r#"
        responders = 100
        responder-requests = 100
        js-runtime-heap-size = 10485760
        js-runtime-script-execution-time = 30000
    "#,
        )
        .unwrap();
        assert_eq!(config, SubscriptionWebhooksConfig::default());
    }
}
