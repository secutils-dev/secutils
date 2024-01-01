use serde::{Deserialize, Serialize};

/// The extended key usage indicates one or more purposes for which the public key may be used, in
/// addition to or in place of the basic purposes indicated in the key usage.
/// See https://www.ietf.org/rfc/rfc5280.html
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ExtendedKeyUsage {
    CodeSigning,
    EmailProtection,
    TimeStamping,
    TlsWebClientAuthentication,
    TlsWebServerAuthentication,
}

#[cfg(test)]
mod tests {
    use crate::utils::certificates::ExtendedKeyUsage;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(ExtendedKeyUsage::CodeSigning, @r###""codeSigning""###);
        assert_json_snapshot!(ExtendedKeyUsage::EmailProtection, @r###""emailProtection""###);
        assert_json_snapshot!(ExtendedKeyUsage::TimeStamping, @r###""timeStamping""###);
        assert_json_snapshot!(ExtendedKeyUsage::TlsWebClientAuthentication, @r###""tlsWebClientAuthentication""###);
        assert_json_snapshot!(ExtendedKeyUsage::TlsWebServerAuthentication, @r###""tlsWebServerAuthentication""###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<ExtendedKeyUsage>(r#""codeSigning""#)?,
            ExtendedKeyUsage::CodeSigning
        );

        assert_eq!(
            serde_json::from_str::<ExtendedKeyUsage>(r#""emailProtection""#)?,
            ExtendedKeyUsage::EmailProtection
        );

        assert_eq!(
            serde_json::from_str::<ExtendedKeyUsage>(r#""timeStamping""#)?,
            ExtendedKeyUsage::TimeStamping
        );

        assert_eq!(
            serde_json::from_str::<ExtendedKeyUsage>(r#""tlsWebClientAuthentication""#)?,
            ExtendedKeyUsage::TlsWebClientAuthentication
        );

        assert_eq!(
            serde_json::from_str::<ExtendedKeyUsage>(r#""tlsWebServerAuthentication""#)?,
            ExtendedKeyUsage::TlsWebServerAuthentication
        );

        Ok(())
    }
}
