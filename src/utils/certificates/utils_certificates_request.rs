use crate::utils::certificates::x509::{PublicKeyAlgorithm, SignatureAlgorithm};
use serde_derive::Deserialize;
use time::OffsetDateTime;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsCertificatesRequest {
    #[serde(rename_all = "camelCase")]
    GenerateCa {
        common_name: Option<String>,
        country: Option<String>,
        state_or_province: Option<String>,
        locality: Option<String>,
        organization: Option<String>,
        organizational_unit: Option<String>,
        public_key_algorithm: PublicKeyAlgorithm,
        signature_algorithm: SignatureAlgorithm,
        #[serde(with = "time::serde::timestamp")]
        not_valid_before: OffsetDateTime,
        #[serde(with = "time::serde::timestamp")]
        not_valid_after: OffsetDateTime,
        version: u8,
    },
    GenerateRsaKeyPair,
}
