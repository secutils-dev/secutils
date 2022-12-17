use crate::server::app_state::AppState;
use actix_web::{web, HttpResponse, Responder};
use serde_derive::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateParams {
    pub activation_code: String,
}

pub async fn security_users_activate(
    state: web::Data<AppState>,
    body_params: web::Json<ActivateParams>,
) -> impl Responder {
    let body_params = body_params.into_inner();
    if body_params.activation_code.is_empty() {
        return HttpResponse::BadRequest().json(json!({ "status": "failed" }));
    }

    match state.api.users().activate(&body_params.activation_code) {
        Ok(user) => {
            log::info!("Successfully activated user: {:?}", user);
            HttpResponse::Ok().json(json!({ "status": "ok" }))
        }
        Err(err) => {
            log::error!(
                "Failed to activate user with code {}: {:?}",
                body_params.activation_code,
                err
            );
            HttpResponse::InternalServerError().json(json!({ "status": "failed" }))
        }
    }
}
