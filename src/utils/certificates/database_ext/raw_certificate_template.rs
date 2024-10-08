use crate::utils::certificates::CertificateTemplate;
use time::OffsetDateTime;
use uuid::Uuid;

use super::raw_certificate_attributes::RawCertificateAttributes;

#[derive(Debug, Eq, PartialEq, Clone)]
pub(super) struct RawCertificateTemplate {
    pub id: Uuid,
    pub name: String,
    pub attributes: Vec<u8>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl TryFrom<RawCertificateTemplate> for CertificateTemplate {
    type Error = anyhow::Error;

    fn try_from(raw: RawCertificateTemplate) -> Result<Self, Self::Error> {
        Ok(CertificateTemplate {
            id: raw.id,
            name: raw.name,
            attributes: postcard::from_bytes::<RawCertificateAttributes>(&raw.attributes)?.into(),
            created_at: raw.created_at,
            updated_at: raw.updated_at,
        })
    }
}

impl TryFrom<&CertificateTemplate> for RawCertificateTemplate {
    type Error = anyhow::Error;

    fn try_from(item: &CertificateTemplate) -> Result<Self, Self::Error> {
        Ok(RawCertificateTemplate {
            id: item.id,
            name: item.name.clone(),
            attributes: postcard::to_stdvec(&RawCertificateAttributes::from(
                item.attributes.clone(),
            ))?,
            created_at: item.created_at,
            updated_at: item.updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::RawCertificateTemplate;
    use crate::utils::certificates::{
        CertificateAttributes, CertificateTemplate, ExtendedKeyUsage, KeyUsage,
        PrivateKeyAlgorithm, SignatureAlgorithm, Version,
    };
    use time::OffsetDateTime;
    use uuid::uuid;

    #[test]
    fn can_convert_into_certificate_template() -> anyhow::Result<()> {
        assert_eq!(
            CertificateTemplate::try_from(RawCertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                attributes: vec![
                    1, 2, 99, 110, 1, 1, 99, 1, 1, 115, 0, 0, 0, 3, 0, 160, 31, 1, 10, 0, 0, 0, 0,
                    0, 0, 180, 31, 1, 10, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 5, 1, 1, 1
                ],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                // January 1, 2000 10:00:10
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            })?,
            CertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                attributes: CertificateAttributes {
                    common_name: Some("cn".to_string()),
                    country: Some("c".to_string()),
                    state_or_province: Some("s".to_string()),
                    locality: None,
                    organization: None,
                    organizational_unit: None,
                    key_algorithm: PrivateKeyAlgorithm::Ed25519,
                    signature_algorithm: SignatureAlgorithm::Md5,
                    not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
                    not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
                    version: Version::One,
                    is_ca: true,
                    key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                    extended_key_usage: Some(
                        [ExtendedKeyUsage::EmailProtection].into_iter().collect()
                    ),
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_into_raw_certificate_template() -> anyhow::Result<()> {
        assert_eq!(
            RawCertificateTemplate::try_from(&CertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                attributes: CertificateAttributes {
                    common_name: Some("cn".to_string()),
                    country: Some("c".to_string()),
                    state_or_province: Some("s".to_string()),
                    locality: None,
                    organization: None,
                    organizational_unit: None,
                    key_algorithm: PrivateKeyAlgorithm::Ed25519,
                    signature_algorithm: SignatureAlgorithm::Md5,
                    not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
                    not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
                    version: Version::One,
                    is_ca: true,
                    key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                    extended_key_usage: Some(
                        [ExtendedKeyUsage::EmailProtection].into_iter().collect()
                    ),
                },
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            })?,
            RawCertificateTemplate {
                id: uuid!("00000000-0000-0000-0000-000000000001"),
                name: "pk-name".to_string(),
                attributes: vec![
                    1, 2, 99, 110, 1, 1, 99, 1, 1, 115, 0, 0, 0, 3, 0, 160, 31, 1, 10, 0, 0, 0, 0,
                    0, 0, 180, 31, 1, 10, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 5, 1, 1, 1
                ],
                // January 1, 2000 10:00:00
                created_at: OffsetDateTime::from_unix_timestamp(946720800)?,
                // January 1, 2000 10:00:10
                updated_at: OffsetDateTime::from_unix_timestamp(946720810)?,
            }
        );

        Ok(())
    }
}
