use crate::{
    security::Operator,
    server::{StatusLevel, app_state::AppState},
};
use actix_web::{HttpResponse, Responder, error::ErrorInternalServerError, web};
use anyhow::anyhow;
use serde::Deserialize;
use tracing::error;

#[derive(Deserialize)]
pub struct SetStatusAPIParams {
    pub level: StatusLevel,
}

pub async fn status_set(
    state: web::Data<AppState>,
    body_params: web::Json<SetStatusAPIParams>,
    operator: Operator,
) -> impl Responder {
    state
        .status
        .write()
        .map(|mut status| {
            status.level = body_params.level;
            HttpResponse::NoContent().finish()
        })
        .map_err(|err| {
            error!(
                operator = operator.id(),
                "Failed to set server status: {err:?}."
            );
            ErrorInternalServerError(anyhow!("Failed to set server status: {:?}.", err))
        })
}
