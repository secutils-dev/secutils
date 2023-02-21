use serde_derive::{Deserialize, Serialize};
use webauthn_rs::prelude::{PasskeyAuthentication, PasskeyRegistration};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum WebAuthnSessionValue {
    RegistrationState(PasskeyRegistration),
    AuthenticationState(PasskeyAuthentication),
}
