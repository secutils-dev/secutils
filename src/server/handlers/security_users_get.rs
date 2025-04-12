use crate::{
    logging::UserLogContext,
    security::Operator,
    server::{AppState, http_errors::generic_internal_server_error},
    users::UserId,
};
use actix_web::{Error, HttpResponse, Responder, web};

pub async fn security_users_get(
    state: web::Data<AppState>,
    operator: Operator,
    user_id: web::Path<UserId>,
) -> impl Responder {
    Ok::<HttpResponse, Error>(match state.api.users().get(*user_id).await {
        Ok(Some(user_to_retrieve)) => HttpResponse::Ok().json(user_to_retrieve),
        Ok(None) => HttpResponse::NotFound().finish(),
        Err(err) => {
            log::error!(
                operator:serde = operator.id(),
                user:serde = UserLogContext::new(*user_id);
                "Failed to retrieve user by ID: {err:?}"
            );
            generic_internal_server_error()
        }
    })
}
