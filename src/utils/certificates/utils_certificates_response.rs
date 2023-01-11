use serde_derive::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsCertificatesResponse {
    #[serde(rename_all = "camelCase")]
    GenerateSelfSignedCertificate {
        private_key: Vec<u8>,
        public_key: Vec<u8>,
        cert: Vec<u8>,
    },
    GenerateRsaKeyPair(Vec<u8>),
}
