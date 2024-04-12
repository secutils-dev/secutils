use serde_derive::Deserialize;

/// Traits represent an identity's traits. The identity is able to create, modify, and delete traits
/// in a self-service manner. The input will always be validated against the JSON Schema.
#[derive(Debug, PartialEq, Deserialize)]
pub struct IdentityTraits {
    /// Main user email address.
    pub email: String,
}

#[cfg(test)]
mod tests {
    use crate::security::kratos::IdentityTraits;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<IdentityTraits>(
                r#"
{
    "email": "dev@secutils.dev"
}
          "#
            )?,
            IdentityTraits {
                email: "dev@secutils.dev".to_string()
            }
        );

        Ok(())
    }
}
