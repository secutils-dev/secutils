use crate::utils::{
    CertificateAttributes, ExtendedKeyUsage, KeyUsage, SignatureAlgorithm, Version,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use time::OffsetDateTime;

use super::raw_private_key_algorithm::RawPrivateKeyAlgorithm;

/// Main `CertificateAttributes` struct has Serde attributes that are needed for JSON serialization,
/// but aren't compatible with the `postcard`. This struct "copy" is used only for the `postcard`
/// serialization and deserialization.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub(super) struct RawCertificateAttributes {
    pub common_name: Option<String>,
    pub country: Option<String>,
    pub state_or_province: Option<String>,
    pub locality: Option<String>,
    pub organization: Option<String>,
    pub organizational_unit: Option<String>,
    pub key_algorithm: RawPrivateKeyAlgorithm,
    pub signature_algorithm: SignatureAlgorithm,
    pub not_valid_before: OffsetDateTime,
    pub not_valid_after: OffsetDateTime,
    pub version: Version,
    pub is_ca: bool,
    pub key_usage: Option<HashSet<KeyUsage>>,
    pub extended_key_usage: Option<HashSet<ExtendedKeyUsage>>,
}

impl From<RawCertificateAttributes> for CertificateAttributes {
    fn from(raw: RawCertificateAttributes) -> Self {
        // Destructure all attributes to catch new or changed fields at compile-time.
        let RawCertificateAttributes {
            common_name,
            country,
            state_or_province,
            locality,
            organization,
            organizational_unit,
            key_algorithm,
            signature_algorithm,
            not_valid_before,
            not_valid_after,
            version,
            is_ca,
            key_usage,
            extended_key_usage,
        } = raw;

        CertificateAttributes {
            common_name,
            country,
            state_or_province,
            locality,
            organization,
            organizational_unit,
            key_algorithm: key_algorithm.into(),
            signature_algorithm,
            not_valid_before,
            not_valid_after,
            version,
            is_ca,
            key_usage,
            extended_key_usage,
        }
    }
}

impl From<CertificateAttributes> for RawCertificateAttributes {
    fn from(item: CertificateAttributes) -> Self {
        // Destructure all attributes to catch new or changed fields at compile-time.
        let CertificateAttributes {
            common_name,
            country,
            state_or_province,
            locality,
            organization,
            organizational_unit,
            key_algorithm,
            signature_algorithm,
            not_valid_before,
            not_valid_after,
            version,
            is_ca,
            key_usage,
            extended_key_usage,
        } = item;

        RawCertificateAttributes {
            common_name,
            country,
            state_or_province,
            locality,
            organization,
            organizational_unit,
            key_algorithm: key_algorithm.into(),
            signature_algorithm,
            not_valid_before,
            not_valid_after,
            version,
            is_ca,
            key_usage,
            extended_key_usage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RawCertificateAttributes;
    use crate::utils::{
        certificates::database_ext::raw_private_key_algorithm::RawPrivateKeyAlgorithm,
        CertificateAttributes, ExtendedKeyUsage, KeyUsage, PrivateKeyAlgorithm, SignatureAlgorithm,
        Version,
    };
    use time::OffsetDateTime;

    #[test]
    fn can_convert_to_certificate_attributes() -> anyhow::Result<()> {
        let not_valid_before = OffsetDateTime::from_unix_timestamp(946720800)?;
        let not_valid_after = OffsetDateTime::from_unix_timestamp(1262340000)?;

        assert_eq!(
            CertificateAttributes::from(RawCertificateAttributes {
                common_name: Some("cn".to_string()),
                country: Some("c".to_string()),
                state_or_province: Some("s".to_string()),
                locality: Some("l".to_string()),
                organization: Some("o".to_string()),
                organizational_unit: Some("ou".to_string()),
                key_algorithm: RawPrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Md5,
                not_valid_before,
                not_valid_after,
                version: Version::One,
                is_ca: true,
                key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
            }),
            CertificateAttributes {
                common_name: Some("cn".to_string()),
                country: Some("c".to_string()),
                state_or_province: Some("s".to_string()),
                locality: Some("l".to_string()),
                organization: Some("o".to_string()),
                organizational_unit: Some("ou".to_string()),
                key_algorithm: PrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Md5,
                not_valid_before,
                not_valid_after,
                version: Version::One,
                is_ca: true,
                key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
            }
        );

        assert_eq!(
            CertificateAttributes::from(RawCertificateAttributes {
                common_name: Some("cn".to_string()),
                country: Some("c".to_string()),
                state_or_province: Some("s".to_string()),
                locality: Some("l".to_string()),
                organization: Some("o".to_string()),
                organizational_unit: None,
                key_algorithm: RawPrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Md5,
                not_valid_before,
                not_valid_after,
                version: Version::One,
                is_ca: true,
                key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
            }),
            CertificateAttributes {
                common_name: Some("cn".to_string()),
                country: Some("c".to_string()),
                state_or_province: Some("s".to_string()),
                locality: Some("l".to_string()),
                organization: Some("o".to_string()),
                organizational_unit: None,
                key_algorithm: PrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Md5,
                not_valid_before,
                not_valid_after,
                version: Version::One,
                is_ca: true,
                key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
            }
        );

        Ok(())
    }

    #[test]
    fn can_convert_to_raw_certificate_attributes() -> anyhow::Result<()> {
        let not_valid_before = OffsetDateTime::from_unix_timestamp(946720800)?;
        let not_valid_after = OffsetDateTime::from_unix_timestamp(1262340000)?;

        assert_eq!(
            RawCertificateAttributes::from(CertificateAttributes {
                common_name: Some("cn".to_string()),
                country: Some("c".to_string()),
                state_or_province: Some("s".to_string()),
                locality: Some("l".to_string()),
                organization: Some("o".to_string()),
                organizational_unit: Some("ou".to_string()),
                key_algorithm: PrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Md5,
                not_valid_before,
                not_valid_after,
                version: Version::One,
                is_ca: true,
                key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
            }),
            RawCertificateAttributes {
                common_name: Some("cn".to_string()),
                country: Some("c".to_string()),
                state_or_province: Some("s".to_string()),
                locality: Some("l".to_string()),
                organization: Some("o".to_string()),
                organizational_unit: Some("ou".to_string()),
                key_algorithm: RawPrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Md5,
                not_valid_before,
                not_valid_after,
                version: Version::One,
                is_ca: true,
                key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
            }
        );

        assert_eq!(
            RawCertificateAttributes::from(CertificateAttributes {
                common_name: Some("cn".to_string()),
                country: Some("c".to_string()),
                state_or_province: Some("s".to_string()),
                locality: Some("l".to_string()),
                organization: Some("o".to_string()),
                organizational_unit: None,
                key_algorithm: PrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Md5,
                not_valid_before,
                not_valid_after,
                version: Version::One,
                is_ca: true,
                key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
            }),
            RawCertificateAttributes {
                common_name: Some("cn".to_string()),
                country: Some("c".to_string()),
                state_or_province: Some("s".to_string()),
                locality: Some("l".to_string()),
                organization: Some("o".to_string()),
                organizational_unit: None,
                key_algorithm: RawPrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Md5,
                not_valid_before,
                not_valid_after,
                version: Version::One,
                is_ca: true,
                key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
            }
        );

        Ok(())
    }

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::to_stdvec(&RawCertificateAttributes {
                common_name: Some("cn".to_string()),
                country: Some("c".to_string()),
                state_or_province: Some("s".to_string()),
                locality: Some("l".to_string()),
                organization: Some("o".to_string()),
                organizational_unit: Some("ou".to_string()),
                key_algorithm: RawPrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Md5,
                not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
                not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
                version: Version::One,
                is_ca: true,
                key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
            })?,
            vec![
                1, 2, 99, 110, 1, 1, 99, 1, 1, 115, 1, 1, 108, 1, 1, 111, 1, 2, 111, 117, 3, 0,
                160, 31, 1, 10, 0, 0, 0, 0, 0, 0, 180, 31, 1, 10, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 5,
                1, 1, 1
            ]
        );

        assert_eq!(
            postcard::to_stdvec(&RawCertificateAttributes {
                common_name: Some("cn".to_string()),
                country: Some("c".to_string()),
                state_or_province: Some("s".to_string()),
                locality: None,
                organization: None,
                organizational_unit: None,
                key_algorithm: RawPrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Md5,
                not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
                not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
                version: Version::One,
                is_ca: true,
                key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
            })?,
            vec![
                1, 2, 99, 110, 1, 1, 99, 1, 1, 115, 0, 0, 0, 3, 0, 160, 31, 1, 10, 0, 0, 0, 0, 0,
                0, 180, 31, 1, 10, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 5, 1, 1, 1
            ]
        );

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::from_bytes::<RawCertificateAttributes>(&[
                1, 2, 99, 110, 1, 1, 99, 1, 1, 115, 1, 1, 108, 1, 1, 111, 1, 2, 111, 117, 3, 0,
                160, 31, 1, 10, 0, 0, 0, 0, 0, 0, 180, 31, 1, 10, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 5,
                1, 1, 1
            ])?,
            RawCertificateAttributes {
                common_name: Some("cn".to_string()),
                country: Some("c".to_string()),
                state_or_province: Some("s".to_string()),
                locality: Some("l".to_string()),
                organization: Some("o".to_string()),
                organizational_unit: Some("ou".to_string()),
                key_algorithm: RawPrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Md5,
                not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
                not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
                version: Version::One,
                is_ca: true,
                key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
            }
        );

        assert_eq!(
            postcard::from_bytes::<RawCertificateAttributes>(&[
                1, 2, 99, 110, 1, 1, 99, 1, 1, 115, 0, 0, 0, 3, 0, 160, 31, 1, 10, 0, 0, 0, 0, 0,
                0, 180, 31, 1, 10, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 5, 1, 1, 1
            ])?,
            RawCertificateAttributes {
                common_name: Some("cn".to_string()),
                country: Some("c".to_string()),
                state_or_province: Some("s".to_string()),
                locality: None,
                organization: None,
                organizational_unit: None,
                key_algorithm: RawPrivateKeyAlgorithm::Ed25519,
                signature_algorithm: SignatureAlgorithm::Md5,
                not_valid_before: OffsetDateTime::from_unix_timestamp(946720800)?,
                not_valid_after: OffsetDateTime::from_unix_timestamp(1262340000)?,
                version: Version::One,
                is_ca: true,
                key_usage: Some([KeyUsage::KeyAgreement].into_iter().collect()),
                extended_key_usage: Some([ExtendedKeyUsage::EmailProtection].into_iter().collect()),
            }
        );

        Ok(())
    }
}
