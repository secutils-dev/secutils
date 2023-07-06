use crate::{
    error::SecutilsError,
    server::app_state::AppState,
    users::User,
    utils::{UtilsAction, UtilsActionHandler},
};
use actix_web::{web, HttpResponse};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct BodyParams {
    action: UtilsAction,
}

pub async fn utils_handle_action(
    state: web::Data<AppState>,
    user: User,
    body_params: web::Json<BodyParams>,
) -> Result<HttpResponse, SecutilsError> {
    let action = body_params.into_inner().action;
    if let Err(err) = action.validate() {
        log::error!("Invalid utility action (user ID: {:?}): {}", user.id, err);
        return Ok(HttpResponse::BadRequest().json(err.to_string()));
    }

    Ok(HttpResponse::Ok().json(UtilsActionHandler::handle(user, &state.api, action).await?))
}
