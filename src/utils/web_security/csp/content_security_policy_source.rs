use serde::{Deserialize, Serialize};

/// Defines a source by means of which content security policy is delivered.
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ContentSecurityPolicySource {
    /// Indicates that a policy is delivered via `Content-Security-Policy` HTTP header.
    EnforcingHeader,
    /// Indicates that a policy is delivered via `Content-Security-Policy-Report-Only` HTTP header.
    ReportOnlyHeader,
    /// Indicates that a policy is delivered via `<meta>` HTML tag.
    Meta,
}

impl ContentSecurityPolicySource {
    /// Name of the HTTP header or `http-equiv` attribute value that is used to deliver a policy.
    pub fn header_name(&self) -> &str {
        match self {
            ContentSecurityPolicySource::EnforcingHeader | ContentSecurityPolicySource::Meta => {
                "content-security-policy"
            }
            ContentSecurityPolicySource::ReportOnlyHeader => "content-security-policy-report-only",
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::ContentSecurityPolicySource;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ContentSecurityPolicySource::EnforcingHeader, @r###""enforcingHeader""###);
        assert_json_snapshot!(ContentSecurityPolicySource::ReportOnlyHeader, @r###""reportOnlyHeader""###);
        assert_json_snapshot!(ContentSecurityPolicySource::Meta, @r###""meta""###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySource>(r#""enforcingHeader""#)?,
            ContentSecurityPolicySource::EnforcingHeader
        );
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySource>(r#""reportOnlyHeader""#)?,
            ContentSecurityPolicySource::ReportOnlyHeader
        );
        assert_eq!(
            serde_json::from_str::<ContentSecurityPolicySource>(r#""meta""#)?,
            ContentSecurityPolicySource::Meta
        );

        Ok(())
    }

    #[test]
    fn correctly_returns_header_name() -> anyhow::Result<()> {
        assert_eq!(
            ContentSecurityPolicySource::EnforcingHeader.header_name(),
            "content-security-policy"
        );
        assert_eq!(
            ContentSecurityPolicySource::ReportOnlyHeader.header_name(),
            "content-security-policy-report-only"
        );
        assert_eq!(
            ContentSecurityPolicySource::Meta.header_name(),
            "content-security-policy"
        );

        Ok(())
    }
}
