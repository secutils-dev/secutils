use serde::{Deserialize, Serialize};

/// See https://www.w3.org/TR/trusted-types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ContentSecurityPolicyRequireTrustedTypesForDirectiveValue {
    #[serde(rename = "'script'")]
    Script,
}

#[cfg(test)]
mod tests {
    use super::ContentSecurityPolicyRequireTrustedTypesForDirectiveValue;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(
            ContentSecurityPolicyRequireTrustedTypesForDirectiveValue::Script,
            @r###""'script'""###
        );

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyRequireTrustedTypesForDirectiveValue>(
                r#""'script'""#
            )?,
            ContentSecurityPolicyRequireTrustedTypesForDirectiveValue::Script
        );

        Ok(())
    }
}
