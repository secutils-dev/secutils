use crate::utils::{PublicKeyAlgorithm, SignatureAlgorithm};
use serde::{Deserialize, Serialize};
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
    #[serde(rename = "pka")]
    pub public_key_algorithm: PublicKeyAlgorithm,
    #[serde(rename = "sa")]
    pub signature_algorithm: SignatureAlgorithm,
    #[serde(rename = "nb", with = "time::serde::timestamp")]
    pub not_valid_before: OffsetDateTime,
    #[serde(rename = "na", with = "time::serde::timestamp")]
    pub not_valid_after: OffsetDateTime,
    #[serde(rename = "v")]
    pub version: u8,
    #[serde(rename = "ca")]
    pub is_ca: bool,
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        tests::MockSelfSignedCertificate, PublicKeyAlgorithm, SelfSignedCertificate,
        SignatureAlgorithm,
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
                "name",
                PublicKeyAlgorithm::Rsa,
                SignatureAlgorithm::Sha256,
                not_valid_before,
                not_valid_after,
                1,
            ).build(),
            @r###"
        {
          "n": "name",
          "pka": "rsa",
          "sa": "sha256",
          "nb": 946720800,
          "na": 1262340000,
          "v": 1,
          "ca": false
        }
        "###
        );
        assert_json_snapshot!(
            MockSelfSignedCertificate::new(
                "test-2-name",
                PublicKeyAlgorithm::Ed25519,
                SignatureAlgorithm::Ed25519,
                not_valid_before,
                not_valid_after,
                3,
            )
            .set_is_ca()
            .set_common_name("CA Issuer")
            .set_country("US")
            .set_state_or_province("California")
            .set_locality("San Francisco")
            .set_organization("CA Issuer, Inc")
            .set_organization_unit("CA Org Unit")
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
          "pka": "ed25519",
          "sa": "ed25519",
          "nb": 946720800,
          "na": 1262340000,
          "v": 3,
          "ca": true
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
                r###"
        {
          "n": "name",
          "pka": "rsa",
          "sa": "sha256",
          "nb": 946720800,
          "na": 1262340000,
          "v": 1,
          "ca": false
        }
        "###
            )?,
            MockSelfSignedCertificate::new(
                "name",
                PublicKeyAlgorithm::Rsa,
                SignatureAlgorithm::Sha256,
                not_valid_before,
                not_valid_after,
                1,
            )
            .build()
        );
        assert_eq!(
            serde_json::from_str::<SelfSignedCertificate>(
                r###"
        {
          "n": "test-2-name",
          "cn": "CA Issuer",
          "c": "US",
          "s": "California",
          "l": "San Francisco",
          "o": "CA Issuer, Inc",
          "ou": "CA Org Unit",
          "pka": "ed25519",
          "sa": "ed25519",
          "nb": 946720800,
          "na": 1262340000,
          "v": 3,
          "ca": true
        }
        "###
            )?,
            MockSelfSignedCertificate::new(
                "test-2-name",
                PublicKeyAlgorithm::Ed25519,
                SignatureAlgorithm::Ed25519,
                not_valid_before,
                not_valid_after,
                3,
            )
            .set_is_ca()
            .set_common_name("CA Issuer")
            .set_country("US")
            .set_state_or_province("California")
            .set_locality("San Francisco")
            .set_organization("CA Issuer, Inc")
            .set_organization_unit("CA Org Unit")
            .build()
        );

        Ok(())
    }
}
