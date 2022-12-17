use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PublicKeyAlgorithm {
    Rsa,
    Dsa,
    Ecdsa,
    Ed25519,
}
