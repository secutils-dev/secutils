use crate::{
    authentication::StoredCredentials,
    server::{app_state::AppState, http_errors::generic_internal_server_error},
};
use actix_http::HttpMessage;
use actix_identity::Identity;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde_derive::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct SignupParams {
    pub email: String,
    pub password: String,
}

/// Signups user with email and password.
pub async fn security_signup(
    state: web::Data<AppState>,
    request: HttpRequest,
    body_params: web::Json<SignupParams>,
) -> impl Responder {
    let body_params = body_params.into_inner();
    if !mailchecker::is_valid(&body_params.email) {
        log::error!("Invalid email was used for signup: {}", body_params.email);
        return HttpResponse::BadRequest().json(json!({
            "message": "Email appears to be invalid or sent from a disposable/throwaway email service."
        }));
    }

    if body_params.password.is_empty() || body_params.password.len() < 8 {
        log::error!("Invalid password was used for signup.");
        return HttpResponse::BadRequest()
            .json(json!({ "message": "Password cannot be empty or shorter than 8 characters." }));
    }

    let credentials = match StoredCredentials::try_from_password(&body_params.password) {
        Ok(credentials) => credentials,
        Err(err) => {
            log::error!(
                "Password doesn't meet minimal security constraints: {:?}",
                err
            );
            return HttpResponse::BadRequest()
                .json(json!({ "message": "Password doesn't meet minimal security constraints." }));
        }
    };

    let users_api = state.api.users();
    match users_api.get_by_email(&body_params.email).await {
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

    let user = match users_api.signup(&body_params.email, credentials).await {
        Ok(user) => {
            log::info!("Successfully signed up user: {}", user.handle);
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
