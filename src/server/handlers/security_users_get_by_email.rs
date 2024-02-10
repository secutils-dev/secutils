use crate::{
    server::{http_errors::generic_internal_server_error, AppState},
    users::User,
};
use actix_web::{web, Error, HttpResponse, Responder};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Query {
    email: String,
}

pub async fn security_users_get_by_email(
    state: web::Data<AppState>,
    user: User,
    query: web::Query<Query>,
) -> impl Responder {
    state.ensure_admin(&user)?;

    Ok::<HttpResponse, Error>(match state.api.users().get_by_email(&query.email).await {
        Ok(Some(user_to_retrieve)) => HttpResponse::Ok().json(user_to_retrieve),
        Ok(None) => HttpResponse::NotFound().finish(),
        Err(err) => {
            log::error!("Failed to retrieve user by email: {err:?}");
            generic_internal_server_error()
        }
    })
}
