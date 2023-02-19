use crate::users::UserWebAuthnSessionValue;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct UserWebAuthnSession {
    pub email: String,
    pub value: UserWebAuthnSessionValue,
    pub timestamp: OffsetDateTime,
}
