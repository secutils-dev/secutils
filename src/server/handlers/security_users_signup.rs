use crate::server::app_state::AppState;
use actix_http::HttpMessage;
use actix_identity::Identity;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde_derive::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct SignupParams {
    pub username: String,
    pub password: String,
}

pub async fn security_users_signup(
    state: web::Data<AppState>,
    body_params: web::Json<SignupParams>,
    request: HttpRequest,
) -> impl Responder {
    let body_params = body_params.into_inner();
    if body_params.username.is_empty()
        || !body_params.username.contains('@')
        || body_params.password.is_empty()
    {
        return HttpResponse::BadRequest().json(json!({ "status": "failed" }));
    }

    let user = match state
        .api
        .users()
        .signup(&body_params.username, &body_params.password)
    {
        Ok(user) => {
            log::info!("Successfully signed up user: {:?}", user);
            user
        }
        Err(err) => {
            log::error!(
                "Failed to signup user (`{}`): {:?}",
                body_params.username,
                err
            );
            return HttpResponse::InternalServerError().json(json!({ "status": "failed" }));
        }
    };

    match Identity::login(&request.extensions(), user.email) {
        Ok(identity) => {
            log::debug!(
                "Logged in user (`{}`) as {:?}",
                body_params.username,
                identity.id()
            );
            HttpResponse::Ok().json(json!({ "status": "ok" }))
        }
        Err(err) => {
            log::error!(
                "Failed to log in user (`{}`): {:?}",
                body_params.username,
                err
            );
            HttpResponse::Unauthorized().json(json!({ "status": "failed" }))
        }
    }
}
