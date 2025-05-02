use crate::{
    security::Operator,
    server::{app_state::AppState, http_errors::generic_internal_server_error},
};
use actix_web::{Error, HttpResponse, Responder, web};
use serde::Deserialize;
use serde_json::json;
use tracing::{error, info, warn};

#[derive(Deserialize)]
pub struct RemoveParams {
    pub email: String,
}

pub async fn security_users_remove(
    state: web::Data<AppState>,
    body_params: web::Json<RemoveParams>,
    operator: Operator,
) -> impl Responder {
    let body_params = body_params.into_inner();
    if body_params.email.is_empty() {
        return Ok::<HttpResponse, Error>(
            HttpResponse::BadRequest().json(json!({ "message": "The email cannot be empty." })),
        );
    }

    let api = state.api.security();
    match api.terminate(&body_params.email).await {
        Ok(Some(user_id)) => {
            info!(
                operator = operator.id(),
                user.id = %user_id,
                "Successfully removed user.",
            );
        }
        Ok(None) => {
            warn!(operator = operator.id(), "Cannot remove non-existent user.");
        }
        Err(err) => {
            error!(operator = operator.id(), "Failed to remove user: {err:?}");
            return Ok(generic_internal_server_error());
        }
    }

    Ok(HttpResponse::NoContent().finish())
}
