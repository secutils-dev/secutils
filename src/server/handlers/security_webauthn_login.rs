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
    authentication::WEBAUTHN_SESSION_KEY,
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
use webauthn_rs::prelude::PublicKeyCredential;

#[derive(Deserialize)]
pub struct LoginStartParams {
    pub email: String,
}

/// The initial stage of the WebAuthn authentication flow.
pub async fn security_webauthn_login_start(
    state: web::Data<AppState>,
    session: Session,
    body_params: web::Json<LoginStartParams>,
) -> impl Responder {
    let body_params = body_params.into_inner();
    if body_params.email.is_empty() {
        log::error!("Invalid email was used for login: {}", body_params.email);
        return HttpResponse::BadRequest().json(json!({
            "message": "This email appears to be invalid."
        }));
    }

    // Remove any previous registrations that may have occurred from the session.
    let users_api = state.api.users();
    if let Some(email) = session.remove(WEBAUTHN_SESSION_KEY) {
        if let Err(err) = users_api.remove_webauthn_session_by_email(&email).await {
            log::error!("Failed to remove WebAuthn session: {:?}", err);
            return generic_internal_server_error();
        }
    }

    // Retrieve user using provided email.
    let user = match users_api.get_by_email(&body_params.email).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            log::error!("User is not found (`{}`).", body_params.email);
            return HttpResponse::Unauthorized().json(json!({ "message": "Failed to authenticate user. Please check your credentials and try again, or contact us for assistance." }));
        }
        Err(err) => {
            log::error!("Failed to retrieve user: {:?}", err);
            return generic_internal_server_error();
        }
    };

    // Make sure the user has passkey credentials configured.
    let passkey = if let Some(passkey) = user.credentials.passkey {
        passkey
    } else {
        log::error!("User doesn't have a passkey configured.",);
        return HttpResponse::Unauthorized().json(json!({ "message": "Failed to authenticate user. Please check your credentials and try again, or contact us for assistance." }));
    };

    // Generate challenge response.
    let (ccr, auth_state) = match state.webauthn.start_passkey_authentication(&[passkey]) {
        Ok(authentication) => authentication,
        Err(err) => {
            log::error!("Failed to start WebAuthn authentication: {:?}", err);
            return generic_internal_server_error();
        }
    };

    let webauthn_session_store_result = users_api
        .upsert_webauthn_session(&UserWebAuthnSession {
            email: body_params.email.to_string(),
            value: UserWebAuthnSessionValue::AuthenticationState(auth_state),
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

/// The final stage of the WebAuthn authentication flow.
pub async fn security_webauthn_login_finish(
    state: web::Data<AppState>,
    session: Session,
    request: HttpRequest,
    body_params: web::Json<PublicKeyCredential>,
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

    // Make sure that user is in process of authentication.
    let authentication_state =
        if let UserWebAuthnSessionValue::AuthenticationState(authentication) =
            webauthn_session.value
        {
            authentication
        } else {
            log::error!(
                "WebAuthn session value isn't suitable for authentication: {:?}",
                webauthn_session.value
            );
            return generic_internal_server_error();
        };

    let authentication_result = match state
        .webauthn
        .finish_passkey_authentication(&body_params, &authentication_state)
    {
        Ok(authentication_result) => authentication_result,
        Err(err) => {
            log::error!(
                "Failed to finish WebAuthn authentication (`{}`): {:?}",
                email,
                err
            );
            return HttpResponse::InternalServerError().json(json!({ "status": "failed" }));
        }
    };

    // Update credentials counter to protect against cloned authenticators.
    if authentication_result.needs_update() {
        todo!()
    }

    // Clear WebAuthn session state since we no longer need it.
    if let Err(err) = users_api.remove_webauthn_session_by_email(&email).await {
        log::error!("Failed to clear WebAuthn session: {:?}", err);
        return generic_internal_server_error();
    }

    match Identity::login(&request.extensions(), email.clone()) {
        Ok(identity) => {
            log::debug!("Logged in user (`{}`) as {:?}", email, identity.id());
            HttpResponse::Ok().finish()
        }
        Err(err) => {
            log::error!("Failed to log in user (`{}`): {:?}", email, err);
            generic_internal_server_error()
        }
    }
}
