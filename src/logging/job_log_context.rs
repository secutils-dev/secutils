use serde::Serialize;
use uuid::Uuid;

/// Represents a context for the job used for the structured logging.
#[derive(Serialize, Debug, Copy, Clone, PartialEq)]
pub struct JobLogContext {
    /// Unique id of the job.
    pub id: Uuid,
}

impl JobLogContext {
    /// Returns context used for the structured logging.
    pub fn new(id: Uuid) -> Self {
        Self { id }
    }
}

#[cfg(test)]
mod tests {
    use crate::logging::JobLogContext;
    use insta::assert_json_snapshot;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(JobLogContext::new(uuid!("00000000-0000-0000-0000-000000000001")), @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001"
        }
        "###);

        Ok(())
    }
}
