use actix_identity::Identity;
use actix_web::{HttpResponse, Responder};

pub async fn security_logout(identity: Option<Identity>) -> impl Responder {
    if let Some(user) = identity {
        user.logout();
    }
    HttpResponse::NoContent()
        // We might need to NOT send this header for Chrome-based browser because of https://bugs.chromium.org/p/chromium/issues/detail?id=762417,
        // but let's see if anyone complains about that yet.
        .append_header(("Clear-Site-Data", r#""cache", "cookies""#))
        .finish()
}
