use crate::{
    server::{app_state::AppState, status::StatusLevel},
    users::User,
};
use actix_web::{error::ErrorInternalServerError, web, HttpResponse, Responder};
use anyhow::anyhow;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SetStatusAPIParams {
    pub level: StatusLevel,
}

pub async fn status_set(
    state: web::Data<AppState>,
    body_params: web::Json<SetStatusAPIParams>,
    user: User,
) -> impl Responder {
    state.ensure_admin(&user)?;

    state
        .status
        .write()
        .map(|mut status| {
            status.level = body_params.level;
            HttpResponse::NoContent().finish()
        })
        .map_err(|err| ErrorInternalServerError(anyhow!("Failed to set server status: {:?}.", err)))
}
