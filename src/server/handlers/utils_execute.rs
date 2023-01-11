use crate::{
    error::SecutilsError,
    server::app_state::AppState,
    users::User,
    utils::{UtilsExecutor, UtilsRequest},
};
use actix_web::{web, HttpResponse};
use serde_derive::Deserialize;

#[derive(Deserialize)]
pub struct BodyParams {
    request: UtilsRequest,
}

pub async fn utils_execute(
    state: web::Data<AppState>,
    user: User,
    body_params: web::Json<BodyParams>,
) -> Result<HttpResponse, SecutilsError> {
    Ok(HttpResponse::Ok()
        .json(UtilsExecutor::execute(user, &state.api, body_params.into_inner().request).await?))
}
