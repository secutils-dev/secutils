use crate::{
    security::Credentials,
    server::{app_state::AppState, http_errors::generic_internal_server_error},
};
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPasswordParams {
    pub email: String,
    pub password: String,
    pub reset_code: String,
}

/// Resets user password.
pub async fn security_credentials_reset_password(
    state: web::Data<AppState>,
    body_params: web::Json<ResetPasswordParams>,
) -> impl Responder {
    let body_params = body_params.into_inner();
    if body_params.email.is_empty() {
        log::error!("Empty email was used for password reset.");
        return HttpResponse::BadRequest().json(json!({ "message": "The email cannot be empty." }));
    }

    if body_params.password.is_empty() || body_params.password.len() < 8 {
        log::error!("Invalid password was used for password reset.");
        return HttpResponse::BadRequest()
            .json(json!({ "message": "Password cannot be empty or shorter than 8 characters." }));
    }

    match state
        .security
        .reset_credentials(
            &body_params.email,
            Credentials::Password(body_params.password),
            &body_params.reset_code,
        )
        .await
    {
        Ok(user) => {
            log::info!("Successfully reset user ({}) password.", *user.id);
            HttpResponse::NoContent().finish()
        }
        Err(err) => {
            log::error!("Failed to update user password: {:?}", err);
            generic_internal_server_error()
        }
    }
}
