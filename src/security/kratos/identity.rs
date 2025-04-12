use crate::security::kratos::{IdentityTraits, IdentityVerifiableAddress};
use serde_derive::Deserialize;
use time::OffsetDateTime;
use uuid::Uuid;

/// Kratos Identity struct, see https://www.ory.sh/kratos/docs/reference/api#models-identity
#[derive(Debug, PartialEq, Deserialize)]
pub struct Identity {
    /// Identity's unique identifier.
    pub id: Uuid,
    /// Identity's traits that can be managed by the identity themselves.
    pub traits: IdentityTraits,
    /// Contains all the addresses that can be verified by the user.
    pub verifiable_addresses: Vec<IdentityVerifiableAddress>,
    /// When the identity was created.
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl Identity {
    /// Determines if the identity is activated.
    pub fn is_activated(&self) -> bool {
        self.verifiable_addresses
            .iter()
            .any(|address| address.value == self.traits.email && address.verified)
    }
}

#[cfg(test)]
mod tests {
    use crate::security::kratos::{Identity, IdentityTraits, IdentityVerifiableAddress};
    use time::OffsetDateTime;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<Identity>(
                r#"
{
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
          "#
            )?,
            Identity {
                id: "f7f3b3b4-3b0b-4b3b-8b3b-3b3b3b3b3b3b".parse()?,
                traits: IdentityTraits {
                    email: "dev@secutils.dev".to_string()
                },
                verifiable_addresses: vec![IdentityVerifiableAddress {
                    value: "dev@secutils.dev".to_string(),
                    verified: true,
                }],
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );

        Ok(())
    }

    #[test]
    fn activated() -> anyhow::Result<()> {
        assert!(
            Identity {
                id: "f7f3b3b4-3b0b-4b3b-8b3b-3b3b3b3b3b3b".parse()?,
                traits: IdentityTraits {
                    email: "dev@secutils.dev".to_string()
                },
                verifiable_addresses: vec![
                    IdentityVerifiableAddress {
                        value: "dev1@secutils.dev".to_string(),
                        verified: false,
                    },
                    IdentityVerifiableAddress {
                        value: "dev@secutils.dev".to_string(),
                        verified: true,
                    }
                ],
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
            .is_activated()
        );

        assert!(
            !Identity {
                id: "f7f3b3b4-3b0b-4b3b-8b3b-3b3b3b3b3b3b".parse()?,
                traits: IdentityTraits {
                    email: "dev@secutils.dev".to_string()
                },
                verifiable_addresses: vec![
                    IdentityVerifiableAddress {
                        value: "dev@secutils.dev".to_string(),
                        verified: false,
                    },
                    IdentityVerifiableAddress {
                        value: "dev1@secutils.dev".to_string(),
                        verified: false,
                    },
                    IdentityVerifiableAddress {
                        value: "dev2@secutils.dev".to_string(),
                        verified: true,
                    }
                ],
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
            .is_activated()
        );

        Ok(())
    }
}
