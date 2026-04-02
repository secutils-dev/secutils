use serde::Serialize;
use utoipa::ToSchema;

/// Database connectivity status.
#[derive(Clone, Serialize, ToSchema)]
pub struct DatabaseStatus {
    /// Indicates if the database is reachable.
    pub operational: bool,
}

#[cfg(test)]
mod tests {
    use crate::server::DatabaseStatus;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(DatabaseStatus {
            operational: true,
        }, @r###"
        {
          "operational": true
        }
        "###);

        assert_json_snapshot!(DatabaseStatus {
            operational: false,
        }, @r###"
        {
          "operational": false
        }
        "###);

        Ok(())
    }
}
