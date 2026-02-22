use serde::{Deserialize, Deserializer, Serialize, de};

/// See https://www.w3.org/TR/trusted-types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContentSecurityPolicyTrustedTypesDirectiveValue {
    #[serde(rename = "'allow-duplicates'")]
    AllowDuplicates,
    #[serde(rename = "'none'")]
    None,
    #[serde(rename = "*")]
    Wildcard,
    #[serde(untagged, deserialize_with = "deserialize_custom_directive_value")]
    PolicyName(String),
}

/// A custom deserialization function for custom directive values.
fn deserialize_custom_directive_value<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let policy_name = String::deserialize(deserializer)?;
    if !policy_name.is_empty() {
        Ok(policy_name)
    } else {
        Err(de::Error::invalid_value(
            de::Unexpected::Str(&policy_name),
            &"[non-empty-alpha-numeric-string]",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::ContentSecurityPolicyTrustedTypesDirectiveValue;
    use insta::{assert_debug_snapshot, assert_json_snapshot};

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(
            ContentSecurityPolicyTrustedTypesDirectiveValue::AllowDuplicates,
            @r###""'allow-duplicates'""###
        );
        assert_json_snapshot!(
            ContentSecurityPolicyTrustedTypesDirectiveValue::None,
            @r###""'none'""###
        );
        assert_json_snapshot!(
            ContentSecurityPolicyTrustedTypesDirectiveValue::Wildcard,
            @r###""*""###
        );
        assert_json_snapshot!(
            ContentSecurityPolicyTrustedTypesDirectiveValue::PolicyName("some-policy-name".to_string()),
            @r###""some-policy-name""###
        );

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyTrustedTypesDirectiveValue>(
                r#""'allow-duplicates'""#
            )?,
            ContentSecurityPolicyTrustedTypesDirectiveValue::AllowDuplicates
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyTrustedTypesDirectiveValue>(r#""'none'""#)?,
            ContentSecurityPolicyTrustedTypesDirectiveValue::None
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyTrustedTypesDirectiveValue>(r#""*""#)?,
            ContentSecurityPolicyTrustedTypesDirectiveValue::Wildcard
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyTrustedTypesDirectiveValue>(
                r#""some-policy-name""#
            )?,
            ContentSecurityPolicyTrustedTypesDirectiveValue::PolicyName(
                "some-policy-name".to_string()
            )
        );

        Ok(())
    }

    #[test]
    fn fails_if_policy_name_is_invalid() -> anyhow::Result<()> {
        assert_debug_snapshot!(
            serde_json::from_str::<ContentSecurityPolicyTrustedTypesDirectiveValue>(
                r#""""#
            ), @r###"
        Err(
            Error("data did not match any variant of untagged enum ContentSecurityPolicyTrustedTypesDirectiveValue", line: 0, column: 0),
        )
        "###);

        Ok(())
    }
}
