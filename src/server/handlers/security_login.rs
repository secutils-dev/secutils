use crate::{
    authentication::Credentials,
    server::{app_state::AppState, http_errors::generic_internal_server_error},
};
use actix_http::HttpMessage;
use actix_identity::Identity;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde_derive::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct LoginParams {
    pub email: String,
    pub password: String,
}

pub async fn security_login(
    state: web::Data<AppState>,
    body_params: web::Json<LoginParams>,
    request: HttpRequest,
) -> impl Responder {
    let body_params = body_params.into_inner();
    if body_params.email.is_empty() {
        log::error!("Invalid email was used for login: {}", body_params.email);
        return HttpResponse::BadRequest().json(json!({
            "message": "This email appears to be invalid."
        }));
    }

    if body_params.password.is_empty() {
        log::error!("Invalid password was used for login.");
        return HttpResponse::BadRequest().json(json!({ "message": "Password cannot be empty." }));
    }

    let users_api = state.api.users();
    let user = match users_api
        .authenticate(
            &body_params.email,
            Credentials::Password(body_params.password),
        )
        .await
    {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to log in user: {:?}", err);
            return HttpResponse::Unauthorized().json(json!({ "message": "Authentication failed. Please check your credentials and try again, or contact us for assistance." }));
        }
    };

    match Identity::login(&request.extensions(), user.email.clone()) {
        Ok(_) => {
            log::debug!("Successfully logged in user (`{}`).", user.handle);
            HttpResponse::Ok().json(json!({ "user": user }))
        }
        Err(err) => {
            log::error!("Failed to log in user (`{}`): {:?}", user.handle, err);
            generic_internal_server_error()
        }
    }
}
