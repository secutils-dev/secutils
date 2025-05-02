use crate::{error::Error as SecutilsError, server::app_state::AppState};
use actix_web::{HttpResponse, web};
use anyhow::anyhow;
use std::ops::Deref;
use tracing::error;

pub async fn status_get(state: web::Data<AppState>) -> Result<HttpResponse, SecutilsError> {
    state
        .status
        .read()
        .map(|status| HttpResponse::Ok().json(status.deref()))
        .map_err(|err| {
            error!("Failed to read status: {err}");
            SecutilsError::from(anyhow!("Status is not available."))
        })
}
