use crate::users::UserWebAuthnSessionValue;

#[derive(Debug, Clone)]
pub struct UserWebAuthnSession {
    pub email: String,
    pub value: UserWebAuthnSessionValue,
}
