use serde_derive::{Deserialize, Serialize};
use serde_with::{DurationSeconds, serde_as};
use std::time::Duration;

/// Configuration for the database connection.
#[serde_as]
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DatabaseConfig {
    /// Name of the database to connect to.
    pub name: String,
    /// Hostname to use to connect to the database.
    pub host: String,
    /// Port to use to connect to the database.
    pub port: u16,
    /// Username to use to connect to the database.
    pub username: String,
    /// Optional password to use to connect to the database.
    pub password: Option<String>,
    /// Maximum number of connections in the pool. Default is 100.
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Minimum number of connections to maintain in the pool. Default is 5.
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
    /// Maximum time to wait for a connection from the pool. Default is 10 seconds.
    #[serde_as(as = "DurationSeconds<u64>")]
    #[serde(default = "default_acquire_timeout")]
    pub acquire_timeout: Duration,
    /// Maximum lifetime of a connection. Default is 30 minutes.
    #[serde_as(as = "DurationSeconds<u64>")]
    #[serde(default = "default_max_lifetime")]
    pub max_lifetime: Duration,
    /// Maximum idle time for a connection before it is closed. Default is 10 minutes.
    #[serde_as(as = "DurationSeconds<u64>")]
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: Duration,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            name: "secutils".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            username: "postgres".to_string(),
            password: None,
            max_connections: default_max_connections(),
            min_connections: default_min_connections(),
            acquire_timeout: default_acquire_timeout(),
            max_lifetime: default_max_lifetime(),
            idle_timeout: default_idle_timeout(),
        }
    }
}

const fn default_max_connections() -> u32 {
    100
}

const fn default_min_connections() -> u32 {
    5
}

const fn default_acquire_timeout() -> Duration {
    Duration::from_secs(10)
}

const fn default_max_lifetime() -> Duration {
    Duration::from_secs(30 * 60)
}

const fn default_idle_timeout() -> Duration {
    Duration::from_secs(10 * 60)
}

#[cfg(test)]
mod tests {
    use crate::config::DatabaseConfig;
    use insta::{assert_debug_snapshot, assert_toml_snapshot};

    #[test]
    fn serialization() {
        let config = DatabaseConfig::default();
        assert_toml_snapshot!(config, @r###"
        name = 'secutils'
        host = 'localhost'
        port = 5432
        username = 'postgres'
        max_connections = 100
        min_connections = 5
        acquire_timeout = 10
        max_lifetime = 1800
        idle_timeout = 600
        "###);

        let config = DatabaseConfig {
            password: Some("password".to_string()),
            ..Default::default()
        };
        assert_toml_snapshot!(config, @r###"
        name = 'secutils'
        host = 'localhost'
        port = 5432
        username = 'postgres'
        password = 'password'
        max_connections = 100
        min_connections = 5
        acquire_timeout = 10
        max_lifetime = 1800
        idle_timeout = 600
        "###);
    }

    #[test]
    fn deserialization() {
        let config: DatabaseConfig = toml::from_str(
            r#"
        name = 'secutils'
        username = 'postgres'
        password = 'password'
        host = 'localhost'
        port = 5432
    "#,
        )
        .unwrap();
        assert_debug_snapshot!(config, @r###"
        DatabaseConfig {
            name: "secutils",
            host: "localhost",
            port: 5432,
            username: "postgres",
            password: Some(
                "password",
            ),
            max_connections: 100,
            min_connections: 5,
            acquire_timeout: 10s,
            max_lifetime: 1800s,
            idle_timeout: 600s,
        }
        "###);
    }
}
