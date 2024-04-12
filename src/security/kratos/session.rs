use crate::security::kratos::Identity;
use serde_derive::Deserialize;
use uuid::Uuid;

/// Kratos Session struct, see https://www.ory.sh/docs/kratos/reference/api#tag/identity/operation/getSession.
#[derive(Debug, PartialEq, Deserialize)]
pub struct Session {
    /// Unique identifier of the session.
    pub id: Uuid,
    /// The identity that is authenticated in this session.
    pub identity: Option<Identity>,
}

#[cfg(test)]
mod tests {
    use crate::security::kratos::{Identity, IdentityTraits, IdentityVerifiableAddress, Session};
    use time::OffsetDateTime;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<Session>(
                r#"
{
    "id": "f8f3b3b4-3b0b-4b3b-8b3b-3b3b3b3b3b3b",
    "identity": {
        "id": "f7f3b3b4-3b0b-4b3b-8b3b-3b3b3b3b3b3b",
        "traits": {
            "email": "dev@secutils.dev"
        },
        "verifiable_addresses": [{
            "value": "dev@secutils.dev",
            "verified": true
        }],
        "created_at": "2000-01-01T10:00:00Z"
    }
}
          "#
            )?,
            Session {
                id: "f8f3b3b4-3b0b-4b3b-8b3b-3b3b3b3b3b3b".parse()?,
                identity: Some(Identity {
                    id: "f7f3b3b4-3b0b-4b3b-8b3b-3b3b3b3b3b3b".parse()?,
                    traits: IdentityTraits {
                        email: "dev@secutils.dev".to_string()
                    },
                    verifiable_addresses: vec![IdentityVerifiableAddress {
                        value: "dev@secutils.dev".to_string(),
                        verified: true,
                    }],
                    created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                })
            }
        );

        assert_eq!(
            serde_json::from_str::<Session>(
                r#"
{
    "id": "f8f3b3b4-3b0b-4b3b-8b3b-3b3b3b3b3b3b"
}
          "#
            )?,
            Session {
                id: "f8f3b3b4-3b0b-4b3b-8b3b-3b3b3b3b3b3b".parse()?,
                identity: None
            }
        );

        Ok(())
    }
}
