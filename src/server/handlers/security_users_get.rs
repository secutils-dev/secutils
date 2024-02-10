use crate::{
    logging::UserLogContext,
    server::{http_errors::generic_internal_server_error, AppState},
    users::{User, UserId},
};
use actix_web::{web, Error, HttpResponse, Responder};

pub async fn security_users_get(
    state: web::Data<AppState>,
    user: User,
    user_id: web::Path<UserId>,
) -> impl Responder {
    state.ensure_admin(&user)?;

    Ok::<HttpResponse, Error>(match state.api.users().get(*user_id).await {
        Ok(Some(user_to_retrieve)) => HttpResponse::Ok().json(user_to_retrieve),
        Ok(None) => HttpResponse::NotFound().finish(),
        Err(err) => {
            log::error!(user = log::as_serde!(UserLogContext::new(*user_id)); "Failed to retrieve user by ID: {err:?}");
            generic_internal_server_error()
        }
    })
}
