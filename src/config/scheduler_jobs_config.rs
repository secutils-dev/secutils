use serde_derive::{Deserialize, Serialize};

/// Configuration for the Secutils.dev scheduler jobs.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct SchedulerJobsConfig {
    /// The schedule to use for the `NotificationsSend` job.
    pub notifications_send: String,
    /// The schedule to use for the `WebhooksKvSweep` job (expired responder KV cleanup).
    #[serde(default = "default_webhooks_kv_sweep")]
    pub webhooks_kv_sweep: String,
    /// The schedule to use for the `RespondersNotify` job (responder hit notifications).
    #[serde(default = "default_responders_notify")]
    pub responders_notify: String,
}

fn default_webhooks_kv_sweep() -> String {
    "0 */5 * * * *".to_string()
}

fn default_responders_notify() -> String {
    "0 * * * * *".to_string()
}

impl Default for SchedulerJobsConfig {
    fn default() -> Self {
        Self {
            notifications_send: "0/30 * * * * *".to_string(),
            webhooks_kv_sweep: default_webhooks_kv_sweep(),
            responders_notify: default_responders_notify(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::SchedulerJobsConfig;
    use insta::assert_toml_snapshot;

    #[test]
    fn serialization_and_default() {
        assert_toml_snapshot!(SchedulerJobsConfig::default(), @r###"
        notifications_send = '0/30 * * * * *'
        webhooks_kv_sweep = '0 */5 * * * *'
        responders_notify = '0 * * * * *'
        "###);
    }

    #[test]
    fn deserialization() {
        let config: SchedulerJobsConfig = toml::from_str(
            r#"
        notifications_send = '0/30 * * * * *'
        webhooks_kv_sweep = '0 */5 * * * *'
        responders_notify = '0 * * * * *'
    "#,
        )
        .unwrap();
        assert_eq!(config, SchedulerJobsConfig::default());
    }

    #[test]
    fn deserialization_defaults_webhooks_kv_sweep() {
        let config: SchedulerJobsConfig = toml::from_str(
            r#"
        notifications_send = '0/30 * * * * *'
    "#,
        )
        .unwrap();
        assert_eq!(config, SchedulerJobsConfig::default());
    }
}
