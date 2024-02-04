//! Defines handlers used during WebAuthn authentication (copied from https://github.com/kanidm/webauthn-rs/):
//!
//!          ┌───────────────┐     ┌───────────────┐      ┌───────────────┐
//!          │ Authenticator │     │    Browser    │      │     Site      │
//!          └───────────────┘     └───────────────┘      └───────────────┘
//!                  │                     │                      │
//!                  │                     │     1. Start Auth    │
//!                  │                     │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─▶│
//!                  │                     │                      │
//!                  │                     │     2. Challenge     │
//!                  │                     │◀ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┤
//!                  │                     │                      │
//!                  │  3. Select Token    │                      │
//!             ─ ─ ─│◀ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─│                      │
//!  4. Verify │     │                     │                      │
//!                  │    4. Yield Sig     │                      │
//!            └ ─ ─▶│─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─▶                      │
//!                  │                     │    5. Send Auth      │
//!                  │                     │        Opts          │
//!                  │                     │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─▶│─ ─ ─
//!                  │                     │                      │     │ 5. Verify
//!                  │                     │                      │          Sig
//!                  │                     │                      │◀─ ─ ┘
//!                  │                     │                      │
//!                  │                     │                      │
use crate::{
    security::{Credentials, WebAuthnChallengeType, WEBAUTHN_SESSION_KEY},
    server::{app_state::AppState, http_errors::generic_internal_server_error},
};
use actix_identity::Identity;
use actix_session::Session;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse, Responder};
use anyhow::Context;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct SigninStartParams {
    pub email: String,
}

/// The initial stage of the WebAuthn authentication flow.
pub async fn security_webauthn_signin_start(
    state: web::Data<AppState>,
    session: Session,
    body_params: web::Json<SigninStartParams>,
) -> impl Responder {
    let body_params = body_params.into_inner();
    if body_params.email.is_empty() {
        log::error!("Invalid email was used for sign-in: {}", body_params.email);
        return HttpResponse::BadRequest().json(json!({
            "message": "This email appears to be invalid."
        }));
    }

    // Remove any previous authentications that may have occurred from the session.
    session.remove(WEBAUTHN_SESSION_KEY);

    // Start handshake and return challenge to the client.
    let security_api = state.api.security();
    let webauthn_challenge_result = security_api
        .start_webauthn_handshake(&body_params.email, WebAuthnChallengeType::Authentication)
        .await
        .and_then(|challenge| {
            session
                .insert(WEBAUTHN_SESSION_KEY, &body_params.email)
                .with_context(|| "Failed to store WebAuthn session in cookie.")?;
            Ok(challenge)
        });
    match webauthn_challenge_result {
        Ok(challenge) => HttpResponse::Ok().json(challenge),
        Err(err) => {
            log::error!("Failed to start WebAuthn authentication: {:?}", err);
            generic_internal_server_error()
        }
    }
}

/// The final stage of the WebAuthn authentication flow.
pub async fn security_webauthn_signin_finish(
    state: web::Data<AppState>,
    session: Session,
    request: HttpRequest,
    body_params: web::Json<serde_json::Value>,
) -> impl Responder {
    let body_params = body_params.into_inner();

    // Retrieve user email from the cookie first.
    let email = if let Some(Ok(email)) = session.remove_as::<String>(WEBAUTHN_SESSION_KEY) {
        email
    } else {
        log::error!("Cannot find WebAuthn session in the cookie.");
        return generic_internal_server_error();
    };

    let security_api = state.api.security();
    let user = match security_api
        .authenticate(&email, Credentials::WebAuthnPublicKey(body_params))
        .await
    {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to sign in user: {:?}", err);
            return HttpResponse::Unauthorized().json(json!({ "message": "Failed to authenticate user. Please check your credentials and try again, or contact us for assistance." }));
        }
    };

    match Identity::login(&request.extensions(), user.email.clone()) {
        Ok(_) => {
            log::debug!("Successfully signed in user (`{}`).", user.handle);
            HttpResponse::Ok().json(json!({ "user": user }))
        }
        Err(err) => {
            log::error!("Failed to sign in user (`{}`): {:?}", user.handle, err);
            generic_internal_server_error()
        }
    }
}
