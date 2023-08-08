use crate::server::{app_state::AppState, http_errors::generic_internal_server_error};
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateParams {
    pub email: String,
    pub activation_code: String,
}

pub async fn security_activation_complete(
    state: web::Data<AppState>,
    body_params: web::Json<ActivateParams>,
) -> impl Responder {
    let body_params = body_params.into_inner();
    if body_params.activation_code.is_empty() || body_params.email.is_empty() {
        return HttpResponse::BadRequest()
            .json(json!({ "message": "User email and activation code should not be empty." }));
    }

    match state
        .security
        .activate(&body_params.email, &body_params.activation_code)
        .await
    {
        Ok(user) => {
            log::info!("Successfully activated user (user ID: {:?}).", user.id);
            HttpResponse::Ok().finish()
        }
        Err(err) => {
            log::error!(
                "Failed to activate user with code {}: {:?}",
                body_params.activation_code,
                err
            );
            generic_internal_server_error()
        }
    }
}
