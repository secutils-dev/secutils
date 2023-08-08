/// Represents a type of the WebAuthn challenge that is generated before WebAuthn registration or
/// authentication handshake.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum WebAuthnChallengeType {
    Registration,
    Authentication,
}
