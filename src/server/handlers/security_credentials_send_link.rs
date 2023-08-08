use crate::server::{app_state::AppState, http_errors::generic_internal_server_error};
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct SendLinkParams {
    pub email: String,
}

/// Sends link to the specified email that allows user to reset their credentials.
pub async fn security_credentials_send_link(
    state: web::Data<AppState>,
    body_params: web::Json<SendLinkParams>,
) -> impl Responder {
    let body_params = body_params.into_inner();
    if body_params.email.is_empty() {
        return HttpResponse::BadRequest().json(json!({ "message": "The email cannot be empty." }));
    }

    let users_api = state.api.users();
    let user = match users_api.get_by_email(&body_params.email).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            // Pretend that sent password reset link. This protection isn't secure enough since by
            // analyzing response time client can figure out if email exists in the database or not.
            return HttpResponse::NoContent().finish();
        }
        Err(err) => {
            log::error!("Failed to retrieve user by email: {:?}", err);
            return generic_internal_server_error();
        }
    };

    match state.security.send_credentials_reset_link(&user).await {
        Ok(_) => {
            log::info!(
                "Successfully sent password reset link (user ID: {:?}).",
                user.id
            );
            HttpResponse::NoContent().finish()
        }
        Err(err) => {
            log::error!(
                "Failed to send password reset link (user ID: {:?}): {:?}",
                user.id,
                err
            );
            generic_internal_server_error()
        }
    }
}
