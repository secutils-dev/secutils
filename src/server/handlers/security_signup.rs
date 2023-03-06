use crate::{
    api::UserSignupError,
    authentication::Credentials,
    server::{app_state::AppState, http_errors::generic_internal_server_error},
};
use actix_http::HttpMessage;
use actix_identity::Identity;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
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

    let users_api = state.api.users();
    let user = match users_api
        .signup(
            &body_params.email,
            Credentials::Password(body_params.password),
        )
        .await
    {
        Ok(user) => {
            log::info!("Successfully signed up user: {}", user.handle);
            user
        }
        Err(err) => {
            log::error!("Failed to signup user: {:?}", err);
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
