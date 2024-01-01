use crate::utils::web_security::ContentSecurityPolicyDirective;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// Represents content security policy (CSP) with the arbitrary name.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContentSecurityPolicy {
    /// Unique content security policy id (UUIDv7).
    pub id: Uuid,
    /// Arbitrary name of the content security policy.
    pub name: String,
    /// Content security policy directives.
    pub directives: Vec<ContentSecurityPolicyDirective>,
    /// Date and time when the content security policy was created.
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_security::{ContentSecurityPolicy, ContentSecurityPolicyDirective};
    use insta::assert_json_snapshot;
    use serde_json::json;
    use std::collections::HashSet;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ContentSecurityPolicy {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            name: "some-name".to_string(),
            directives: vec![
                ContentSecurityPolicyDirective::ChildSrc(["'self'".to_string()].into_iter().collect())
            ],
            // January 1, 2000 11:00:00
            created_at: OffsetDateTime::from_unix_timestamp(946720800)?
        }, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "some-name",
          "directives": [
            {
              "name": "child-src",
              "value": [
                "'self'"
              ]
            }
          ],
          "createdAt": 946720800
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicy>(
                &json!({ "id": "00000000-0000-0000-0000-000000000001", "name": "some-name", "directives": [{"name": "child-src", "value": ["'self'", "https://*"]}], "createdAt": 946720800 })
                    .to_string()
            )?,
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "some-name".to_string(),
                directives: vec![ContentSecurityPolicyDirective::ChildSrc(
                    ["'self'".to_string(), "https://*".to_string()]
                        .into_iter()
                        .collect()
                )],
                // January 1, 2000 11:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            }
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicy>(
                &json!({ "id": "00000000-0000-0000-0000-000000000001" ,"name": "some-name", "directives": [{"name": "sandbox", "value": []}], "createdAt": 946720800 })
                    .to_string()
            )?,
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "some-name".to_string(),
                directives: vec![ContentSecurityPolicyDirective::Sandbox(HashSet::new())],
                // January 1, 2000 11:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?
            }
        );

        Ok(())
    }
}
