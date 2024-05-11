use actix_web::cookie::Cookie;

/// Represents user credentials.
#[derive(Debug, Clone)]
pub enum Credentials {
    /// Kratos session cookie.
    SessionCookie(Cookie<'static>),
    /// JSON Web Token tied to a Kratos identity.
    Jwt(String),
}
