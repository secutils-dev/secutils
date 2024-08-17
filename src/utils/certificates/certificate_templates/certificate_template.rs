use crate::utils::certificates::CertificateAttributes;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

/// Describes stored certificate template.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CertificateTemplate {
    /// Unique certificate template id (UUIDv7).
    pub id: Uuid,
    /// Arbitrary name of the certificate template.
    pub name: String,
    /// Attributes of the certificate that the template defines.
    pub attributes: CertificateAttributes,
    /// Date and time when the certificate template was created.
    #[serde(with = "time::serde::timestamp")]
    pub created_at: OffsetDateTime,
    /// Date and time when the certificate template was last updated.
    #[serde(with = "time::serde::timestamp")]
    pub updated_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::MockCertificateAttributes,
        utils::certificates::{
            CertificateTemplate, PrivateKeyAlgorithm, SignatureAlgorithm, Version,
        },
    };
    use insta::assert_json_snapshot;
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(
            CertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "ct-name".to_string(),
                attributes: MockCertificateAttributes::new(
                    PrivateKeyAlgorithm::Ed25519,
                    SignatureAlgorithm::Ed25519,
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                    OffsetDateTime::from_unix_timestamp(1262340000)?,
                    Version::Three,
                )
                .set_is_ca()
                .set_common_name("CA Issuer")
                .set_country("US")
                .build(),
                // January 1, 2000 11:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                // January 1, 2000 11:00:10
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            },
            @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "ct-name",
          "attributes": {
            "commonName": "CA Issuer",
            "country": "US",
            "keyAlgorithm": {
              "keyType": "ed25519"
            },
            "signatureAlgorithm": "ed25519",
            "notValidBefore": 946720800,
            "notValidAfter": 1262340000,
            "version": 3,
            "isCa": true
          },
          "createdAt": 946720800,
          "updatedAt": 946720810
        }
        "###
        );

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<CertificateTemplate>(
                r#"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "name": "ct-name",
          "attributes": {
            "commonName": "CA Issuer",
            "country": "US",
            "keyAlgorithm": {
              "keyType": "ed25519"
            },
            "signatureAlgorithm": "ed25519",
            "notValidBefore": 946720800,
            "notValidAfter": 1262340000,
            "version": 3,
            "isCa": true
          },
          "createdAt": 946720800,
          "updatedAt": 946720810
        }
        "#
            )?,
            CertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "ct-name".to_string(),
                attributes: MockCertificateAttributes::new(
                    PrivateKeyAlgorithm::Ed25519,
                    SignatureAlgorithm::Ed25519,
                    OffsetDateTime::from_unix_timestamp(946720800)?,
                    OffsetDateTime::from_unix_timestamp(1262340000)?,
                    Version::Three,
                )
                .set_is_ca()
                .set_common_name("CA Issuer")
                .set_country("US")
                .build(),
                // January 1, 2000 11:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                // January 1, 2000 11:00:10
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?
            },
        );

        Ok(())
    }
}
