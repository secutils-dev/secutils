use crate::authentication::WebAuthnSessionValue;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct WebAuthnSession {
    pub email: String,
    pub value: WebAuthnSessionValue,
    pub timestamp: OffsetDateTime,
}
