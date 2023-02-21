/// Represents user credentials.
#[derive(Debug, Clone)]
pub enum Credentials {
    Password(String),
    WebAuthnPublicKey(serde_json::Value),
}
