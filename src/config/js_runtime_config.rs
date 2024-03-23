use serde_derive::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};
use std::time::Duration;

/// Configuration for the JS runtime (Deno).
#[serde_as]
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct JsRuntimeConfig {
    /// The hard limit for the JS runtime heap size in bytes. Defaults to 10485760 bytes or 10 MB.
    pub max_heap_size: usize,
    /// The maximum duration for a single JS script execution. Defaults to 30 seconds.
    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub max_user_script_execution_time: Duration,
}

impl Default for JsRuntimeConfig {
    fn default() -> Self {
        Self {
            // Default value for max size of the heap in bytes is 10 MB.
            max_heap_size: 10485760,
            // Default value for max user script execution time is 30 seconds.
            max_user_script_execution_time: Duration::from_secs(30),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::JsRuntimeConfig;
    use insta::assert_toml_snapshot;

    #[test]
    fn serialization_and_default() {
        assert_toml_snapshot!(JsRuntimeConfig::default(), @r###"
        max-heap-size = 10485760
        max-user-script-execution-time = 30000
        "###);
    }

    #[test]
    fn deserialization() {
        let config: JsRuntimeConfig = toml::from_str(
            r#"
max-heap-size = 10485760
max-user-script-execution-time = 30000
"#,
        )
        .unwrap();
        assert_eq!(config, JsRuntimeConfig::default());
    }
}
