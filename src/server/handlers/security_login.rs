use crate::server::app_state::AppState;
use actix_http::HttpMessage;
use actix_identity::Identity;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde_derive::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct LoginParams {
    pub username: String,
    pub password: String,
}

pub async fn security_login(
    state: web::Data<AppState>,
    body_params: web::Json<LoginParams>,
    request: HttpRequest,
) -> impl Responder {
    let body_params = body_params.into_inner();
    let user = match state
        .api
        .users()
        .authenticate(&body_params.username, &body_params.password)
    {
        Ok(user) => user,
        Err(err) => {
            log::error!("Failed to log in user: {:?}", err);
            return HttpResponse::Unauthorized().json(json!({ "status": "failed" }));
        }
    };

    match Identity::login(&request.extensions(), user.email.clone()) {
        Ok(_) => HttpResponse::Ok().json(json!({ "user": user })),
        Err(err) => {
            log::error!("Failed to log in user: {:?}", err);
            HttpResponse::Unauthorized().json(json!({ "status": "failed" }))
        }
    }
}
