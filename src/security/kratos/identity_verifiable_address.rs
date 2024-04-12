use serde_derive::Deserialize;

/// The address (email or SMS) that can be verified by the user.
#[derive(Debug, PartialEq, Deserialize)]
pub struct IdentityVerifiableAddress {
    /// The address value.
    pub value: String,
    /// Indicates if the address has already been verified
    pub verified: bool,
}

#[cfg(test)]
mod tests {
    use crate::security::kratos::IdentityVerifiableAddress;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<IdentityVerifiableAddress>(
                r#"
{
    "value": "dev@secutils.dev",
    "verified": true
}
          "#
            )?,
            IdentityVerifiableAddress {
                value: "dev@secutils.dev".to_string(),
                verified: true,
            }
        );

        Ok(())
    }
}
