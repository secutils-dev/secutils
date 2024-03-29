use serde_derive::{Deserialize, Serialize};

/// Configuration for the database connection.
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
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
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            name: "secutils".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            username: "postgres".to_string(),
            password: None,
        }
    }
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
        }
        "###);
    }
}
