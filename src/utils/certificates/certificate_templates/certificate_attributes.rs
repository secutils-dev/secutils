use crate::utils::certificates::{
    ExtendedKeyUsage, KeyUsage, PrivateKeyAlgorithm, SignatureAlgorithm, Version,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use time::OffsetDateTime;

/// Describes certificate attributes.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CertificateAttributes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub common_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_or_province: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub organizational_unit: Option<String>,
    pub key_algorithm: PrivateKeyAlgorithm,
    pub signature_algorithm: SignatureAlgorithm,
    #[serde(with = "time::serde::timestamp")]
    pub not_valid_before: OffsetDateTime,
    #[serde(with = "time::serde::timestamp")]
    pub not_valid_after: OffsetDateTime,
    #[serde(default = "Version::latest")]
    pub version: Version,
    pub is_ca: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_usage: Option<HashSet<KeyUsage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extended_key_usage: Option<HashSet<ExtendedKeyUsage>>,
}

#[cfg(test)]
pub mod tests {
    use super::CertificateAttributes;
    use crate::utils::certificates::{
        ExtendedKeyUsage, KeyUsage, PrivateKeyAlgorithm, SignatureAlgorithm, Version,
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;

    pub struct MockCertificateAttributes(CertificateAttributes);
    impl MockCertificateAttributes {
        pub fn new(
            public_key_algorithm: PrivateKeyAlgorithm,
            signature_algorithm: SignatureAlgorithm,
            not_valid_before: OffsetDateTime,
            not_valid_after: OffsetDateTime,
            version: Version,
        ) -> Self {
            Self(CertificateAttributes {
                common_name: None,
                country: None,
                state_or_province: None,
                locality: None,
                organization: None,
                organizational_unit: None,
                key_algorithm: public_key_algorithm,
                signature_algorithm,
                not_valid_before,
                not_valid_after,
                version,
                is_ca: false,
                key_usage: None,
                extended_key_usage: None,
            })
        }

        pub fn set_is_ca(mut self) -> Self {
            self.0.is_ca = true;
            self
        }

        pub fn set_common_name<CN: Into<String>>(mut self, value: CN) -> Self {
            self.0.common_name = Some(value.into());
            self
        }

        pub fn set_country<C: Into<String>>(mut self, value: C) -> Self {
            self.0.country = Some(value.into());
            self
        }

        pub fn set_state_or_province<S: Into<String>>(mut self, value: S) -> Self {
            self.0.state_or_province = Some(value.into());
            self
        }

        pub fn set_locality<L: Into<String>>(mut self, value: L) -> Self {
            self.0.locality = Some(value.into());
            self
        }

        pub fn set_organization<L: Into<String>>(mut self, value: L) -> Self {
            self.0.organization = Some(value.into());
            self
        }

        pub fn set_organization_unit<L: Into<String>>(mut self, value: L) -> Self {
            self.0.organizational_unit = Some(value.into());
            self
        }

        pub fn add_key_usage(mut self, key_usage: KeyUsage) -> Self {
            if let Some(key_usage_list) = self.0.key_usage.as_mut() {
                key_usage_list.insert(key_usage);
            } else {
                self.0.key_usage = Some([key_usage].into_iter().collect());
            }
            self
        }

        pub fn add_extended_key_usage(mut self, key_usage: ExtendedKeyUsage) -> Self {
            if let Some(key_usage_list) = self.0.extended_key_usage.as_mut() {
                key_usage_list.insert(key_usage);
            } else {
                self.0.extended_key_usage = Some([key_usage].into_iter().collect());
            }
            self
        }

        pub fn build(self) -> CertificateAttributes {
            self.0
        }
    }

    #[test]
    fn serialization() -> anyhow::Result<()> {
        // January 1, 2000 11:00:00
        let not_valid_before = OffsetDateTime::from_unix_timestamp(946720800)?;
        // January 1, 2010 11:00:00
        let not_valid_after = OffsetDateTime::from_unix_timestamp(1262340000)?;

        let certificate_attributes = MockCertificateAttributes::new(
            PrivateKeyAlgorithm::Ed25519,
            SignatureAlgorithm::Ed25519,
            not_valid_before,
            not_valid_after,
            Version::Three,
        )
        .set_is_ca()
        .set_common_name("CA Issuer")
        .set_country("US")
        .set_locality("San Francisco")
        .set_organization("CA Issuer, Inc")
        .set_organization_unit("CA Org Unit")
        .set_state_or_province("State")
        .add_key_usage(KeyUsage::CrlSigning)
        .add_extended_key_usage(ExtendedKeyUsage::TlsWebServerAuthentication)
        .build();

        assert_json_snapshot!(certificate_attributes,  @r###"
        {
          "commonName": "CA Issuer",
          "country": "US",
          "stateOrProvince": "State",
          "locality": "San Francisco",
          "organization": "CA Issuer, Inc",
          "organizationalUnit": "CA Org Unit",
          "keyAlgorithm": {
            "keyType": "ed25519"
          },
          "signatureAlgorithm": "ed25519",
          "notValidBefore": 946720800,
          "notValidAfter": 1262340000,
          "version": 3,
          "isCa": true,
          "keyUsage": [
            "crlSigning"
          ],
          "extendedKeyUsage": [
            "tlsWebServerAuthentication"
          ]
        }
        "###);

        let certificate_attributes = MockCertificateAttributes::new(
            PrivateKeyAlgorithm::Ed25519,
            SignatureAlgorithm::Ed25519,
            not_valid_before,
            not_valid_after,
            Version::Three,
        )
        .set_is_ca()
        .set_common_name("CA Issuer")
        .set_country("US")
        .set_locality("San Francisco")
        .add_key_usage(KeyUsage::CrlSigning)
        .add_extended_key_usage(ExtendedKeyUsage::TlsWebServerAuthentication)
        .build();

        assert_json_snapshot!(certificate_attributes,  @r###"
        {
          "commonName": "CA Issuer",
          "country": "US",
          "locality": "San Francisco",
          "keyAlgorithm": {
            "keyType": "ed25519"
          },
          "signatureAlgorithm": "ed25519",
          "notValidBefore": 946720800,
          "notValidAfter": 1262340000,
          "version": 3,
          "isCa": true,
          "keyUsage": [
            "crlSigning"
          ],
          "extendedKeyUsage": [
            "tlsWebServerAuthentication"
          ]
        }
        "###);

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        // January 1, 2000 11:00:00
        let not_valid_before = OffsetDateTime::from_unix_timestamp(946720800)?;
        // January 1, 2010 11:00:00
        let not_valid_after = OffsetDateTime::from_unix_timestamp(1262340000)?;

        assert_eq!(
            serde_json::from_str::<CertificateAttributes>(
                r#"
        {
          "commonName": "CA Issuer",
          "country": "US",
          "stateOrProvince": "State",
          "locality": "San Francisco",
          "organization": "CA Issuer, Inc",
          "organizationalUnit": "CA Org Unit",
          "keyAlgorithm": {
            "keyType": "ed25519"
          },
          "signatureAlgorithm": "ed25519",
          "notValidBefore": 946720800,
          "notValidAfter": 1262340000,
          "version": 3,
          "isCa": true,
          "keyUsage": [
            "crlSigning"
          ],
          "extendedKeyUsage": [
            "tlsWebServerAuthentication"
          ]
        }
        "#
            )?,
            MockCertificateAttributes::new(
                PrivateKeyAlgorithm::Ed25519,
                SignatureAlgorithm::Ed25519,
                not_valid_before,
                not_valid_after,
                Version::Three,
            )
            .set_is_ca()
            .set_common_name("CA Issuer")
            .set_country("US")
            .set_locality("San Francisco")
            .set_organization("CA Issuer, Inc")
            .set_organization_unit("CA Org Unit")
            .set_state_or_province("State")
            .add_key_usage(KeyUsage::CrlSigning)
            .add_extended_key_usage(ExtendedKeyUsage::TlsWebServerAuthentication)
            .build()
        );
        assert_eq!(
            serde_json::from_str::<CertificateAttributes>(
                r#"
        {
          "commonName": "CA Issuer",
          "country": "US",
          "locality": "San Francisco",
          "keyAlgorithm": {
            "keyType": "ed25519"
          },
          "signatureAlgorithm": "ed25519",
          "notValidBefore": 946720800,
          "notValidAfter": 1262340000,
          "version": 3,
          "isCa": true,
          "keyUsage": [
            "crlSigning"
          ],
          "extendedKeyUsage": [
            "tlsWebServerAuthentication"
          ]
        }
        "#
            )?,
            MockCertificateAttributes::new(
                PrivateKeyAlgorithm::Ed25519,
                SignatureAlgorithm::Ed25519,
                not_valid_before,
                not_valid_after,
                Version::Three,
            )
            .set_is_ca()
            .set_common_name("CA Issuer")
            .set_country("US")
            .set_locality("San Francisco")
            .add_key_usage(KeyUsage::CrlSigning)
            .add_extended_key_usage(ExtendedKeyUsage::TlsWebServerAuthentication)
            .build()
        );

        Ok(())
    }
}
