use crate::{
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::User,
};
use actix_web::{web, HttpResponse, Responder};
use serde_json::json;

pub async fn security_activation_send_link(
    state: web::Data<AppState>,
    user: User,
) -> impl Responder {
    if user.activated {
        log::error!(
            "Attempted to activate already activated account (user ID: {:?}).",
            user.id
        );
        return HttpResponse::BadRequest()
            .json(json!({ "message": "User account is already activated." }));
    }

    match state.security.send_activation_link(&user).await {
        Ok(_) => {
            log::info!(
                "Successfully sent account activation link (user ID: {:?}).",
                user.id
            );
            HttpResponse::Ok().finish()
        }
        Err(err) => {
            log::error!(
                "Failed to send account activation link (user ID: {:?}): {:?}",
                user.id,
                err
            );
            generic_internal_server_error()
        }
    }
}
