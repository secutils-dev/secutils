use crate::{error::SecutilsError, server::app_state::AppState, users::User, utils::UtilsAction};
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct BodyParams {
    action: UtilsAction,
}

pub async fn utils_handle_action(
    state: web::Data<AppState>,
    user: User,
    body_params: web::Json<BodyParams>,
) -> Result<HttpResponse, SecutilsError> {
    let user_id = user.id;

    let action = body_params.into_inner().action;
    if let Err(err) = action.validate(&state.network).await {
        log::error!("Invalid utility action (user ID: {:?}): {}", user_id, err);
        return Ok(HttpResponse::BadRequest().json(json!({ "message": err.to_string() })));
    }

    action
        .handle(user, &state.api, &state.network)
        .await
        .map(|response| HttpResponse::Ok().json(response))
        .or_else(|err| {
            log::error!("Failed to execute action (user ID: {:?}): {}", user_id, err);
            Ok(HttpResponse::InternalServerError().json(json!({ "message": err.to_string() })))
        })
}
