use crate::{error::SecutilsError, server::app_state::AppState};
use actix_web::{web, HttpResponse};
use anyhow::anyhow;
use std::ops::Deref;

pub async fn status_get(state: web::Data<AppState>) -> Result<HttpResponse, SecutilsError> {
    state
        .status
        .read()
        .map(|status| HttpResponse::Ok().json(status.deref()))
        .map_err(|err| anyhow!("Failed to retrieve server status: {:?}.", err).into())
}
