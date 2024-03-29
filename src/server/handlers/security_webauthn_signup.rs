//! Defines handlers used during WebAuthn registration (copied from https://github.com/kanidm/webauthn-rs/):
//!
//!          ┌───────────────┐     ┌───────────────┐      ┌───────────────┐
//!          │ Authenticator │     │    Browser    │      │     Site      │
//!          └───────────────┘     └───────────────┘      └───────────────┘
//!                  │                     │                      │
//!                  │                     │     1. Start Reg     │
//!                  │                     │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─▶│
//!                  │                     │                      │
//!                  │                     │     2. Challenge     │
//!                  │                     │◀ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┤
//!                  │                     │                      │
//!                  │  3. Select Token    │                      │
//!             ─ ─ ─│◀ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─│                      │
//!  4. Verify │     │                     │                      │
//!                  │  4. Yield PubKey    │                      │
//!            └ ─ ─▶│─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─▶                      │
//!                  │                     │                      │
//!                  │                     │  5. Send Reg Opts    │
//!                  │                     │─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─▶│─ ─ ─
//!                  │                     │                      │     │ 5. Verify
//!                  │                     │                      │         PubKey
//!                  │                     │                      │◀─ ─ ┘
//!                  │                     │                      │─ ─ ─
//!                  │                     │                      │     │ 6. Persist
//!                  │                     │                      │       Credential
//!                  │                     │                      │◀─ ─ ┘
//!                  │                     │                      │
//!                  │                     │                      │
use crate::{
    security::{Credentials, WebAuthnChallengeType, WEBAUTHN_SESSION_KEY},
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::UserSignupError,
};
use actix_identity::Identity;
use actix_session::Session;
use actix_web::{web, HttpMessage, HttpRequest, HttpResponse, Responder};
use anyhow::Context;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct SignupStartParams {
    pub email: String,
}

/// The initial stage of the WebAuthn registration flow.
pub async fn security_webauthn_signup_start(
    state: web::Data<AppState>,
    session: Session,
    body_params: web::Json<SignupStartParams>,
) -> impl Responder {
    let body_params = body_params.into_inner();
    if !mailchecker::is_valid(&body_params.email) {
        log::warn!("Invalid email was used for signup: {}", body_params.email);
        return HttpResponse::BadRequest().json(json!({
            "message": "Email appears to be invalid or sent from a disposable/throwaway email service."
        }));
    }

    // Remove any previous registrations that may have occurred from the session.
    session.remove(WEBAUTHN_SESSION_KEY);

    // Start handshake and return challenge to the client.
    let security_api = state.api.security();
    let webauthn_challenge_result = security_api
        .start_webauthn_handshake(&body_params.email, WebAuthnChallengeType::Registration)
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
            log::error!("Failed to start WebAuthn registration: {:?}", err);
            generic_internal_server_error()
        }
    }
}

/// The final stage of the WebAuthn registration flow.
pub async fn security_webauthn_signup_finish(
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

    // Finally, create user entry in database
    let security_api = state.api.security();
    let user = match security_api
        .signup(&email, Credentials::WebAuthnPublicKey(body_params))
        .await
    {
        Ok(user) => {
            log::info!("Successfully signed up user (`{}`).", user.handle);
            user
        }
        Err(err) => {
            return match err.downcast_ref::<UserSignupError>() {
                Some(err) => match err {
                    UserSignupError::EmailAlreadyRegistered => HttpResponse::BadRequest().json(
                        json!({ "message": "The email address is already registered. Please try signing in or use a different email address." })
                    )
                },
                None => generic_internal_server_error(),
            };
        }
    };

    match Identity::login(&request.extensions(), user.email) {
        Ok(_) => {
            log::debug!(
                "Successfully signed up and signed in user (`{}`).",
                user.handle
            );
            HttpResponse::Ok().finish()
        }
        Err(err) => {
            log::error!(
                "Failed to sign in user (`{}`) after signup: {:?}",
                user.handle,
                err
            );
            generic_internal_server_error()
        }
    }
}
