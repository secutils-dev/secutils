use crate::{
    security::Operator,
    server::{AppState, http_errors::generic_internal_server_error},
    users::UserId,
};
use actix_web::{Error, HttpResponse, Responder, web};
use tracing::error;

pub async fn security_users_get(
    state: web::Data<AppState>,
    operator: Operator,
    user_id: web::Path<UserId>,
) -> impl Responder {
    Ok::<HttpResponse, Error>(match state.api.users().get(*user_id).await {
        Ok(Some(user_to_retrieve)) => HttpResponse::Ok().json(user_to_retrieve),
        Ok(None) => HttpResponse::NotFound().finish(),
        Err(err) => {
            error!(
                operator = operator.id(),
                user.id = %user_id,
                "Failed to retrieve user by ID: {err:?}"
            );
            generic_internal_server_error()
        }
    })
}
