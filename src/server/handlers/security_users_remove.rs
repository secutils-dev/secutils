use crate::{
    logging::UserLogContext,
    security::Operator,
    server::{app_state::AppState, http_errors::generic_internal_server_error},
};
use actix_web::{web, Error, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;

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

    let users_api = state.api.users();
    match users_api.remove_by_email(&body_params.email).await {
        Ok(Some(user_id)) => {
            log::info!(
                operator:serde = operator.id(),
                user:serde = UserLogContext::new(user_id);
                "Successfully removed user.",
            );
        }
        Ok(None) => {
            log::warn!(operator:serde = operator.id(); "Cannot remove non-existent user.");
        }
        Err(err) => {
            log::error!(operator:serde = operator.id(); "Failed to remove user: {err:?}");
            return Ok(generic_internal_server_error());
        }
    }

    Ok(HttpResponse::NoContent().finish())
}
