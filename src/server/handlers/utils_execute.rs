use crate::{
    error::SecutilsError,
    utils::{UtilsExecutor, UtilsRequest},
};
use actix_web::{web, HttpResponse};
use serde_derive::Deserialize;

#[derive(Deserialize)]
pub struct BodyParams {
    request: UtilsRequest,
}

pub async fn utils_execute(
    body_params: web::Json<BodyParams>,
) -> Result<HttpResponse, SecutilsError> {
    Ok(HttpResponse::Ok().json(UtilsExecutor::execute(body_params.into_inner().request).await?))
}
