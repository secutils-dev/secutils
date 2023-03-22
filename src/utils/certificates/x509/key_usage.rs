use serde::{Deserialize, Serialize};

/// The key usage extension defines the purpose of the public key contained in the certificate.
/// See https://www.ietf.org/rfc/rfc5280.html
#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum KeyUsage {
    CrlSigning,
    DataEncipherment,
    DecipherOnly,
    DigitalSignature,
    EncipherOnly,
    KeyAgreement,
    KeyCertificateSigning,
    KeyEncipherment,
    NonRepudiation,
}

#[cfg(test)]
mod tests {
    use crate::utils::KeyUsage;
    use insta::assert_json_snapshot;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(KeyUsage::CrlSigning, @r###""crlSigning""###);
        assert_json_snapshot!(KeyUsage::DataEncipherment, @r###""dataEncipherment""###);
        assert_json_snapshot!(KeyUsage::DecipherOnly, @r###""decipherOnly""###);
        assert_json_snapshot!(KeyUsage::DigitalSignature, @r###""digitalSignature""###);
        assert_json_snapshot!(KeyUsage::EncipherOnly, @r###""encipherOnly""###);
        assert_json_snapshot!(KeyUsage::KeyAgreement, @r###""keyAgreement""###);
        assert_json_snapshot!(KeyUsage::KeyCertificateSigning, @r###""keyCertificateSigning""###);
        assert_json_snapshot!(KeyUsage::KeyEncipherment, @r###""keyEncipherment""###);
        assert_json_snapshot!(KeyUsage::NonRepudiation, @r###""nonRepudiation""###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<KeyUsage>(r###""crlSigning""###)?,
            KeyUsage::CrlSigning
        );

        assert_eq!(
            serde_json::from_str::<KeyUsage>(r###""dataEncipherment""###)?,
            KeyUsage::DataEncipherment
        );

        assert_eq!(
            serde_json::from_str::<KeyUsage>(r###""decipherOnly""###)?,
            KeyUsage::DecipherOnly
        );

        assert_eq!(
            serde_json::from_str::<KeyUsage>(r###""digitalSignature""###)?,
            KeyUsage::DigitalSignature
        );

        assert_eq!(
            serde_json::from_str::<KeyUsage>(r###""encipherOnly""###)?,
            KeyUsage::EncipherOnly
        );

        assert_eq!(
            serde_json::from_str::<KeyUsage>(r###""keyAgreement""###)?,
            KeyUsage::KeyAgreement
        );

        assert_eq!(
            serde_json::from_str::<KeyUsage>(r###""keyCertificateSigning""###)?,
            KeyUsage::KeyCertificateSigning
        );

        assert_eq!(
            serde_json::from_str::<KeyUsage>(r###""keyEncipherment""###)?,
            KeyUsage::KeyEncipherment
        );

        assert_eq!(
            serde_json::from_str::<KeyUsage>(r###""crlSigning""###)?,
            KeyUsage::CrlSigning
        );

        assert_eq!(
            serde_json::from_str::<KeyUsage>(r###""nonRepudiation""###)?,
            KeyUsage::NonRepudiation
        );

        Ok(())
    }
}
