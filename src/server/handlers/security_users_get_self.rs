use crate::users::User;
use actix_web::{HttpResponse, Responder, get};

/// Returns the currently authenticated user.
#[utoipa::path(
    tags = ["users"],
    responses(
        (status = 200, description = "The authenticated user."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/users/self")]
pub async fn security_users_get_self(user: User) -> impl Responder {
    HttpResponse::Ok().json(user)
}
