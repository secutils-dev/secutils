use serde::{Deserialize, Serialize};

/// Defines a source by means of which content security policy is delivered.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContentSecurityPolicySource {
    /// Indicates that a policy is delivered either via `Content-Security-Policy` or
    /// `Content-Security-Policy-Report-Only` HTTP headers.
    Header,
    /// Indicates that a policy is delivered via `<meta>` HTML tag.
    Meta,
}

#[cfg(test)]
mod tests {
    use crate::utils::ContentSecurityPolicySource;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ContentSecurityPolicySource::Header, @r###""header""###);
        assert_json_snapshot!(ContentSecurityPolicySource::Meta, @r###""meta""###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySource>(r###""header""###)?,
            ContentSecurityPolicySource::Header
        );
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySource>(r###""meta""###)?,
            ContentSecurityPolicySource::Meta
        );

        Ok(())
    }
}
