use crate::users::User;
use actix_web::{HttpResponse, Responder};

pub async fn security_users_get_self(user: User) -> impl Responder {
    HttpResponse::Ok().json(user)
}
