use serde::Deserialize;
use serde_with::{TimestampSeconds, serde_as};
use time::OffsetDateTime;

/// JWT claims struct.
#[serde_as]
#[derive(Debug, Deserialize, Eq, PartialEq)]
pub struct Claims {
    /// User email.
    pub sub: String,
    /// Token expiration time (UTC timestamp).
    #[serde_as(as = "TimestampSeconds<i64>")]
    pub exp: OffsetDateTime,
}

#[cfg(test)]
mod test {
    use crate::security::jwt::Claims;
    use time::OffsetDateTime;

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<Claims>(
                r#"
        {
          "sub": "dev@secutils.dev",
          "exp": 1262340000
        }"#
            )?,
            Claims {
                sub: "dev@secutils.dev".to_string(),
                exp: OffsetDateTime::from_unix_timestamp(1262340000)?,
            }
        );

        Ok(())
    }
}
