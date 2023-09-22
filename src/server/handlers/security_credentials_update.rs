use crate::{
    security::{Credentials, WebAuthnChallengeType},
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::User,
};
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct UpdatePasswordParams {
    pub password: String,
}

/// Updates user password.
pub async fn security_credentials_update_password(
    state: web::Data<AppState>,
    body_params: web::Json<UpdatePasswordParams>,
    user: User,
) -> impl Responder {
    if body_params.password.is_empty() || body_params.password.len() < 8 {
        log::error!("Invalid password was used for password update.");
        return HttpResponse::BadRequest()
            .json(json!({ "message": "Password cannot be empty or shorter than 8 characters." }));
    }

    match state
        .security
        .update_credentials(
            &user.email,
            Credentials::Password(body_params.into_inner().password),
        )
        .await
    {
        Ok(user) => {
            log::info!("Successfully updated user ({}) credentials.", *user.id);
            HttpResponse::NoContent().finish()
        }
        Err(err) => {
            log::error!(
                "Failed to update user ({}) credentials: {:?}",
                *user.id,
                err
            );
            generic_internal_server_error()
        }
    }
}

/// The initial stage of the WebAuthn registration flow.
pub async fn security_credentials_update_passkey_start(
    state: web::Data<AppState>,
    user: User,
) -> impl Responder {
    // Start handshake and return challenge to the client.
    match state
        .security
        .start_webauthn_handshake(&user.email, WebAuthnChallengeType::Registration)
        .await
    {
        Ok(challenge) => HttpResponse::Ok().json(challenge),
        Err(err) => {
            log::error!("Failed to start WebAuthn registration: {:?}", err);
            generic_internal_server_error()
        }
    }
}

/// The final stage of the WebAuthn registration flow.
pub async fn security_credentials_update_passkey_finish(
    state: web::Data<AppState>,
    body_params: web::Json<serde_json::Value>,
    user: User,
) -> impl Responder {
    match state
        .security
        .update_credentials(
            &user.email,
            Credentials::WebAuthnPublicKey(body_params.into_inner()),
        )
        .await
    {
        Ok(user) => {
            log::info!("Successfully updated user ({}) credentials.", *user.id);
            HttpResponse::NoContent().finish()
        }
        Err(err) => {
            log::error!(
                "Failed to update user ({}) credentials: {:?}",
                *user.id,
                err
            );
            generic_internal_server_error()
        }
    }
}
