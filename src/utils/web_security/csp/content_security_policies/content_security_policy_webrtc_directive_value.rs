use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ContentSecurityPolicyWebrtcDirectiveValue {
    #[serde(rename = "'allow'")]
    Allow,
    #[serde(rename = "'block'")]
    Block,
}

#[cfg(test)]
mod tests {
    use crate::utils::web_security::ContentSecurityPolicyWebrtcDirectiveValue;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ContentSecurityPolicyWebrtcDirectiveValue::Allow, @r###""'allow'""###);
        assert_json_snapshot!(ContentSecurityPolicyWebrtcDirectiveValue::Block, @r###""'block'""###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyWebrtcDirectiveValue>(r#""'allow'""#)?,
            ContentSecurityPolicyWebrtcDirectiveValue::Allow
        );

        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicyWebrtcDirectiveValue>(r#""'block'""#)?,
            ContentSecurityPolicyWebrtcDirectiveValue::Block
        );

        Ok(())
    }
}
