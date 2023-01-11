use serde_derive::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsCertificatesRequest {
    #[serde(rename_all = "camelCase")]
    GenerateSelfSignedCertificate {
        template_name: String,
    },
    GenerateRsaKeyPair,
}
