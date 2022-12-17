use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SignatureAlgorithm {
    Md5,
    Sha1,
    Sha256,
    Sha384,
    Sha512,
    Ed25519,
}
