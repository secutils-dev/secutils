use crate::utils::web_security::{ContentSecurityPolicy, ContentSecurityPolicyDirective};
use content_security_policy::{Policy, PolicyDisposition, PolicySource};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawContentSecurityPolicy {
    pub id: Vec<u8>,
    pub name: String,
    pub directives: Vec<u8>,
    pub created_at: i64,
}

impl TryFrom<RawContentSecurityPolicy> for ContentSecurityPolicy {
    type Error = anyhow::Error;

    fn try_from(raw: RawContentSecurityPolicy) -> Result<Self, Self::Error> {
        let directives = postcard::from_bytes::<Vec<String>>(&raw.directives)?
            .into_iter()
            .map(|directive_string| {
                Policy::parse(
                    &directive_string,
                    PolicySource::Header,
                    PolicyDisposition::Enforce,
                )
                .directive_set
                .pop()
                .ok_or_else(|| anyhow::anyhow!("Failed to parse directive: {directive_string}"))
                .and_then(|directive| ContentSecurityPolicyDirective::try_from(&directive))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ContentSecurityPolicy {
            id: Uuid::from_slice(raw.id.as_slice())?,
            name: raw.name,
            directives,
            created_at: OffsetDateTime::from_unix_timestamp(raw.created_at)?,
        })
    }
}

impl TryFrom<&ContentSecurityPolicy> for RawContentSecurityPolicy {
    type Error = anyhow::Error;

    fn try_from(item: &ContentSecurityPolicy) -> Result<Self, Self::Error> {
        let directives = postcard::to_stdvec(
            &item
                .directives
                .iter()
                .map(|directive| String::try_from(directive.clone()))
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        Ok(RawContentSecurityPolicy {
            id: item.id.into(),
            name: item.name.clone(),
            directives,
            created_at: item.created_at.unix_timestamp(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawContentSecurityPolicy;
    use crate::utils::web_security::{
        ContentSecurityPolicy, ContentSecurityPolicyDirective,
        ContentSecurityPolicyTrustedTypesDirectiveValue,
    };
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn can_convert_into_content_security_policy() -> anyhow::Result<()> {
        assert_eq!(
            ContentSecurityPolicy::try_from(RawContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "csp-name".to_string(),
                directives: vec![
                    3, 25, 117, 112, 103, 114, 97, 100, 101, 45, 105, 110, 115, 101, 99, 117, 114,
                    101, 45, 114, 101, 113, 117, 101, 115, 116, 115, 39, 100, 101, 102, 97, 117,
                    108, 116, 45, 115, 114, 99, 32, 39, 115, 101, 108, 102, 39, 32, 104, 116, 116,
                    112, 115, 58, 47, 47, 115, 101, 99, 117, 116, 105, 108, 115, 46, 100, 101, 118,
                    32, 116, 114, 117, 115, 116, 101, 100, 45, 116, 121, 112, 101, 115, 32, 39, 97,
                    108, 108, 111, 119, 45, 100, 117, 112, 108, 105, 99, 97, 116, 101, 115, 39
                ],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            })?,
            ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "csp-name".to_string(),
                directives: vec![
                    ContentSecurityPolicyDirective::UpgradeInsecureRequests,
                    ContentSecurityPolicyDirective::DefaultSrc(
                        ["'self'".to_string(), "https://secutils.dev".to_string()]
                            .into_iter()
                            .collect()
                    ),
                    ContentSecurityPolicyDirective::TrustedTypes(
                        [ContentSecurityPolicyTrustedTypesDirectiveValue::AllowDuplicates]
                            .into_iter()
                            .collect()
                    )
                ],
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_content_security_policy() -> anyhow::Result<()> {
        assert_eq!(
            RawContentSecurityPolicy::try_from(&ContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "csp-name".to_string(),
                directives: vec![
                    ContentSecurityPolicyDirective::UpgradeInsecureRequests,
                    ContentSecurityPolicyDirective::DefaultSrc(
                        ["'self'".to_string(), "https://secutils.dev".to_string()]
                            .into_iter()
                            .collect()
                    ),
                    ContentSecurityPolicyDirective::TrustedTypes(
                        [ContentSecurityPolicyTrustedTypesDirectiveValue::AllowDuplicates]
                            .into_iter()
                            .collect()
                    )
                ],
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
            })?,
            RawContentSecurityPolicy {
                id: uuid!("00000000-0000-0000-0000-000000000001")
                    .as_bytes()
                    .to_vec(),
                name: "csp-name".to_string(),
                directives: vec![
                    3, 25, 117, 112, 103, 114, 97, 100, 101, 45, 105, 110, 115, 101, 99, 117, 114,
                    101, 45, 114, 101, 113, 117, 101, 115, 116, 115, 39, 100, 101, 102, 97, 117,
                    108, 116, 45, 115, 114, 99, 32, 39, 115, 101, 108, 102, 39, 32, 104, 116, 116,
                    112, 115, 58, 47, 47, 115, 101, 99, 117, 116, 105, 108, 115, 46, 100, 101, 118,
                    32, 116, 114, 117, 115, 116, 101, 100, 45, 116, 121, 112, 101, 115, 32, 39, 97,
                    108, 108, 111, 119, 45, 100, 117, 112, 108, 105, 99, 97, 116, 101, 115, 39
                ],
                // January 1, 2000 10:00:00
                created_at: 946720800,
            }
        );

        Ok(())
    }
}
