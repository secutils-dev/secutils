use crate::server::{DatabaseStatus, StatusLevel};
use serde::Serialize;
use utoipa::ToSchema;

/// Server status information.
#[derive(Clone, Serialize, ToSchema)]
pub struct Status {
    /// The server version string.
    pub version: String,
    /// Current availability level.
    pub level: StatusLevel,
    /// Status of the database connection.
    pub db: DatabaseStatus,
}

#[cfg(test)]
mod tests {
    use crate::server::{DatabaseStatus, Status, StatusLevel};
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(Status {
            version: "1.0.0-alpha.4".to_string(),
            level: StatusLevel::Available,
            db: DatabaseStatus { operational: true },
        }, @r###"
        {
          "version": "1.0.0-alpha.4",
          "level": "available",
          "db": {
            "operational": true
          }
        }
        "###);

        Ok(())
    }
}
