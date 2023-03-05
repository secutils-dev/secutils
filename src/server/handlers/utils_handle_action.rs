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
    Ok(HttpResponse::Ok()
        .json(UtilsActionHandler::handle(user, &state.api, body_params.into_inner().action).await?))
}
