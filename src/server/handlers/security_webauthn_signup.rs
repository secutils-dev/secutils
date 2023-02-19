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
    authentication::{StoredCredentials, WEBAUTHN_SESSION_KEY},
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::{UserWebAuthnSession, UserWebAuthnSessionValue},
};
use actix_http::HttpMessage;
use actix_identity::Identity;
use actix_session::Session;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use anyhow::Context;
use serde_derive::Deserialize;
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;
use webauthn_rs::prelude::RegisterPublicKeyCredential;

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
            "message": "Email is not valid or coming from a disposable/throwaway email service."
        }));
    }

    // Remove any previous registrations that may have occurred from the session.
    let users_api = state.api.users();
    if let Some(username) = session.remove(WEBAUTHN_SESSION_KEY) {
        if let Err(err) = users_api.remove_webauthn_session_by_email(&username).await {
            log::error!("Failed to remove WebAuthn session: {:?}", err);
            return generic_internal_server_error();
        }
    }

    // Generate challenge response.
    let (ccr, reg_state) = match state.webauthn.start_passkey_registration(
        Uuid::new_v4(),
        &body_params.email,
        &body_params.email,
        None,
    ) {
        Ok(registration) => registration,
        Err(err) => {
            log::error!("Failed to start WebAuthn registration: {:?}", err);
            return generic_internal_server_error();
        }
    };

    let webauthn_session_store_result = users_api
        .upsert_webauthn_session(&UserWebAuthnSession {
            email: body_params.email.to_string(),
            value: UserWebAuthnSessionValue::RegistrationState(reg_state),
            timestamp: OffsetDateTime::now_utc(),
        })
        .await
        .and_then(|_| {
            session
                .insert(WEBAUTHN_SESSION_KEY, &body_params.email)
                .with_context(|| "Failed to store WebAuthn session in cookie.")
        });
    if let Err(err) = webauthn_session_store_result {
        log::error!("Failed to store WebAuthn session: {:?}", err);
        return generic_internal_server_error();
    }

    HttpResponse::Ok().json(ccr)
}

/// The final stage of the WebAuthn registration flow.
pub async fn security_webauthn_signup_finish(
    state: web::Data<AppState>,
    session: Session,
    request: HttpRequest,
    body_params: web::Json<RegisterPublicKeyCredential>,
) -> impl Responder {
    let body_params = body_params.into_inner();

    // Retrieve user email from the cookie first.
    let email = if let Some(Ok(email)) = session.remove_as::<String>(WEBAUTHN_SESSION_KEY) {
        email
    } else {
        log::error!("Cannot find WebAuthn session in the cookie.");
        return generic_internal_server_error();
    };

    // Then extract stored session state from the DB using user email.
    let users_api = state.api.users();
    let webauthn_session = match users_api.get_webauthn_session_by_email(&email).await {
        Ok(Some(session)) => session,
        Ok(None) => {
            log::error!("Cannot find WebAuthn session in database.");
            return generic_internal_server_error();
        }
        Err(err) => {
            log::error!(
                "Failed to retrieve WebAuthn session from database: {:?}",
                err
            );
            return generic_internal_server_error();
        }
    };

    // Make sure that user is in process of registration.
    let registration_state =
        if let UserWebAuthnSessionValue::RegistrationState(registration) = webauthn_session.value {
            registration
        } else {
            log::error!(
                "WebAuthn session value isn't suitable for registration: {:?}",
                webauthn_session.value
            );
            return generic_internal_server_error();
        };

    // Finish registration and extract passkey.
    let credentials = match state
        .webauthn
        .finish_passkey_registration(&body_params, &registration_state)
    {
        Ok(passkey) => StoredCredentials::from_passkey(passkey),
        Err(err) => {
            log::error!("Failed to finish WebAuthn registration: {:?}", err);
            return generic_internal_server_error();
        }
    };

    // Clear WebAuthn session state since we no longer need it.
    if let Err(err) = users_api.remove_webauthn_session_by_email(&email).await {
        log::error!("Failed to clear WebAuthn session: {:?}", err);
        return generic_internal_server_error();
    }

    // Now we should make sure user isn't registered yet.
    match users_api.get_by_email(&email).await {
        Ok(None) => {
            // User with the provided email doesn't exist yet, move forward.
        }
        Ok(Some(user)) => {
            log::error!("Attempt to register existing user: {}", user.handle);
            return HttpResponse::BadRequest()
                .json(json!({ "message": "User with provided email already registered." }));
        }
        Err(err) => {
            log::error!("Failed to check if user exists: {:?}", err);
            return generic_internal_server_error();
        }
    }

    // Finally, create user entry in database
    let user = match state.api.users().signup(&email, credentials).await {
        Ok(user) => {
            log::info!("Successfully signed up user (`{}`).", user.handle);
            user
        }
        Err(err) => {
            log::error!("Failed to signup user: {:?}", err);
            return generic_internal_server_error();
        }
    };

    match Identity::login(&request.extensions(), user.email) {
        Ok(_) => {
            log::debug!(
                "Successfully signed up and logged in user (`{}`).",
                user.handle
            );
            HttpResponse::Ok().finish()
        }
        Err(err) => {
            log::error!(
                "Failed to log in user (`{}`) after signup: {:?}",
                user.handle,
                err
            );
            generic_internal_server_error()
        }
    }
}
