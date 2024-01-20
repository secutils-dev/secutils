use crate::users::{User, UserId};
use serde::Serialize;

/// Represents a context for the user used for the structured logging.
#[derive(Serialize, Debug, Copy, Clone, PartialEq)]
pub struct UserLogContext {
    /// Unique id of the user.
    pub id: UserId,
}

impl User {
    /// Returns context used for the structured logging.
    pub fn log_context(&self) -> UserLogContext {
        UserLogContext { id: self.id }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        security::StoredCredentials,
        tests::{MockUserBuilder, UserLogContext},
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(UserLogContext {
            id: 1.try_into()?
        }, @r###"
        {
          "id": 1
        }
        "###);

        Ok(())
    }

    #[test]
    fn log_context() -> anyhow::Result<()> {
        let user = MockUserBuilder::new(
            3.try_into()?,
            "my-email",
            "my-handle",
            StoredCredentials {
                password_hash: Some("my-pass-hash".to_string()),
                ..Default::default()
            },
            // January 1, 2010 11:00:00
            OffsetDateTime::from_unix_timestamp(1262340000)?,
        )
        .build();

        assert_eq!(user.log_context(), UserLogContext { id: 3.try_into()? });

        Ok(())
    }
}
