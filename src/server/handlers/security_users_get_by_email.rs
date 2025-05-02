use crate::{
    security::Operator,
    server::{AppState, http_errors::generic_internal_server_error},
};
use actix_web::{Error, HttpResponse, Responder, web};
use serde::Deserialize;
use tracing::error;

#[derive(Deserialize)]
pub struct Query {
    email: String,
}

pub async fn security_users_get_by_email(
    state: web::Data<AppState>,
    operator: Operator,
    query: web::Query<Query>,
) -> impl Responder {
    Ok::<HttpResponse, Error>(match state.api.users().get_by_email(&query.email).await {
        Ok(Some(user_to_retrieve)) => HttpResponse::Ok().json(user_to_retrieve),
        Ok(None) => HttpResponse::NotFound().finish(),
        Err(err) => {
            error!(
                operator = operator.id(),
                "Failed to retrieve user by email: {err:?}"
            );
            generic_internal_server_error()
        }
    })
}
