use crate::utils::{
    certificates::{ExtendedKeyUsage, KeyUsage, Version},
    PrivateKeyAlgorithm, SignatureAlgorithm,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use time::OffsetDateTime;

/// Describes stored self-signed certificate template.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SelfSignedCertificate {
    #[serde(rename = "n")]
    pub name: String,
    #[serde(rename = "cn", skip_serializing_if = "Option::is_none")]
    pub common_name: Option<String>,
    #[serde(rename = "c", skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(rename = "s", skip_serializing_if = "Option::is_none")]
    pub state_or_province: Option<String>,
    #[serde(rename = "l", skip_serializing_if = "Option::is_none")]
    pub locality: Option<String>,
    #[serde(rename = "o", skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    #[serde(rename = "ou", skip_serializing_if = "Option::is_none")]
    pub organizational_unit: Option<String>,
    #[serde(rename = "ka")]
    pub key_algorithm: PrivateKeyAlgorithm,
    #[serde(rename = "sa")]
    pub signature_algorithm: SignatureAlgorithm,
    #[serde(rename = "nb", with = "time::serde::timestamp")]
    pub not_valid_before: OffsetDateTime,
    #[serde(rename = "na", with = "time::serde::timestamp")]
    pub not_valid_after: OffsetDateTime,
    #[serde(rename = "v", default = "Version::latest")]
    pub version: Version,
    #[serde(rename = "ca")]
    pub is_ca: bool,
    #[serde(rename = "ku", skip_serializing_if = "Option::is_none")]
    pub key_usage: Option<HashSet<KeyUsage>>,
    #[serde(rename = "eku", skip_serializing_if = "Option::is_none")]
    pub extended_key_usage: Option<HashSet<ExtendedKeyUsage>>,
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        tests::MockSelfSignedCertificate, ExtendedKeyUsage, KeyUsage, PrivateKeyAlgorithm,
        PrivateKeySize, SelfSignedCertificate, SignatureAlgorithm, Version,
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        // January 1, 2000 11:00:00
        let not_valid_before = OffsetDateTime::from_unix_timestamp(946720800)?;
        // January 1, 2010 11:00:00
        let not_valid_after = OffsetDateTime::from_unix_timestamp(1262340000)?;

        assert_json_snapshot!(
            MockSelfSignedCertificate::new(
                "test-2-name",
                PrivateKeyAlgorithm::Ed25519,
                SignatureAlgorithm::Ed25519,
                not_valid_before,
                not_valid_after,
                Version::Three,
            )
            .set_is_ca()
            .set_common_name("CA Issuer")
            .set_country("US")
            .set_state_or_province("California")
            .set_locality("San Francisco")
            .set_organization("CA Issuer, Inc")
            .set_organization_unit("CA Org Unit")
            .add_key_usage(KeyUsage::CrlSigning)
            .add_extended_key_usage(ExtendedKeyUsage::TlsWebServerAuthentication)
            .build(),
            @r###"
        {
          "n": "test-2-name",
          "cn": "CA Issuer",
          "c": "US",
          "s": "California",
          "l": "San Francisco",
          "o": "CA Issuer, Inc",
          "ou": "CA Org Unit",
          "ka": {
            "keyType": "ed25519"
          },
          "sa": "ed25519",
          "nb": 946720800,
          "na": 1262340000,
          "v": 3,
          "ca": true,
          "ku": [
            "crlSigning"
          ],
          "eku": [
            "tlsWebServerAuthentication"
          ]
        }
        "###
        );
        assert_json_snapshot!(
            MockSelfSignedCertificate::new(
                "name",
                PrivateKeyAlgorithm::Rsa { key_size: PrivateKeySize::Size1024 },
                SignatureAlgorithm::Sha256,
                not_valid_before,
                not_valid_after,
                Version::One,
            ).build(),
            @r###"
        {
          "n": "name",
          "ka": {
            "keyType": "rsa",
            "keySize": "1024"
          },
          "sa": "sha256",
          "nb": 946720800,
          "na": 1262340000,
          "v": 1,
          "ca": false
        }
        "###
        );

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        // January 1, 2000 11:00:00
        let not_valid_before = OffsetDateTime::from_unix_timestamp(946720800)?;
        // January 1, 2010 11:00:00
        let not_valid_after = OffsetDateTime::from_unix_timestamp(1262340000)?;

        assert_eq!(
            serde_json::from_str::<SelfSignedCertificate>(
                r#"
        {
          "n": "name",
          "ka": { "keyType": "rsa", "keySize": "1024" },
          "sa": "sha256",
          "nb": 946720800,
          "na": 1262340000,
          "v": 1,
          "ca": false
        }
        "#
            )?,
            MockSelfSignedCertificate::new(
                "name",
                PrivateKeyAlgorithm::Rsa {
                    key_size: PrivateKeySize::Size1024
                },
                SignatureAlgorithm::Sha256,
                not_valid_before,
                not_valid_after,
                Version::One,
            )
            .build()
        );
        assert_eq!(
            serde_json::from_str::<SelfSignedCertificate>(
                r#"
        {
          "n": "test-2-name",
          "cn": "CA Issuer",
          "c": "US",
          "s": "California",
          "l": "San Francisco",
          "o": "CA Issuer, Inc",
          "ou": "CA Org Unit",
          "ka": { "keyType": "ed25519" },
          "sa": "ed25519",
          "nb": 946720800,
          "na": 1262340000,
          "v": 3,
          "ca": true,
          "ku": ["crlSigning", "keyCertificateSigning"],
          "eku": ["tlsWebServerAuthentication"]
        }
        "#
            )?,
            MockSelfSignedCertificate::new(
                "test-2-name",
                PrivateKeyAlgorithm::Ed25519,
                SignatureAlgorithm::Ed25519,
                not_valid_before,
                not_valid_after,
                Version::latest(),
            )
            .set_is_ca()
            .set_common_name("CA Issuer")
            .set_country("US")
            .set_state_or_province("California")
            .set_locality("San Francisco")
            .set_organization("CA Issuer, Inc")
            .set_organization_unit("CA Org Unit")
            .add_key_usage(KeyUsage::CrlSigning)
            .add_key_usage(KeyUsage::KeyCertificateSigning)
            .add_extended_key_usage(ExtendedKeyUsage::TlsWebServerAuthentication)
            .build()
        );

        Ok(())
    }
}
