use crate::users::User;
use actix_web::{HttpResponse, Responder};

pub async fn user_get(user: User) -> impl Responder {
    HttpResponse::Ok().json(user)
}
