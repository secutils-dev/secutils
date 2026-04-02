use crate::{
    error::Error as SecutilsError,
    server::{DatabaseStatus, StatusLevel, app_state::AppState},
};
use actix_web::{HttpResponse, get, web};
use anyhow::anyhow;
use tracing::error;

/// Returns the current server status.
#[utoipa::path(
    tags = ["status"],
    security(()),
    responses(
        (status = 200, description = "Current server status.", body = crate::server::Status)
    )
)]
#[get("/api/status")]
pub async fn status_get(state: web::Data<AppState>) -> Result<HttpResponse, SecutilsError> {
    let mut status = state.status.read().map(|s| s.clone()).map_err(|err| {
        error!("Failed to read status: {err}");
        SecutilsError::from(anyhow!("Status is not available."))
    })?;

    let db_operational = state.api.db.is_alive().await;
    status.db = DatabaseStatus {
        operational: db_operational,
    };
    if !db_operational {
        error!("Database is not reachable.");
        status.level = StatusLevel::Unavailable;
    }

    Ok(HttpResponse::Ok().json(status))
}
