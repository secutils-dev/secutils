use actix_identity::Identity;
use actix_web::{HttpResponse, Responder};

pub async fn security_logout(identity: Option<Identity>) -> impl Responder {
    if let Some(user) = identity {
        user.logout();
    }
    HttpResponse::NoContent()
        .append_header(("Clear-Site-Data", r#""cache", "cookies""#))
        .finish()
}
