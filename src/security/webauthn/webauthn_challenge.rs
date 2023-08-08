use serde::Serialize;
use webauthn_rs::prelude::*;

/// An opaque JSON serializable WebAuthn challenge which is returned to the user's web browser.
#[derive(Debug, Serialize, Clone)]
pub struct WebAuthnChallenge(serde_json::Value);
impl WebAuthnChallenge {
    /// Creates WebAuthn challenge struct for registration.
    pub fn registration(ccr: &CreationChallengeResponse) -> anyhow::Result<Self> {
        Ok(Self(serde_json::to_value(ccr)?))
    }

    /// Creates WebAuthn challenge struct for authentication.
    pub fn authentication(ccr: &RequestChallengeResponse) -> anyhow::Result<Self> {
        Ok(Self(serde_json::to_value(ccr)?))
    }
}
