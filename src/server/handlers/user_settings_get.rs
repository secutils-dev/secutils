use crate::{
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::{User, UserSettings},
};
use actix_web::{HttpResponse, Responder, get, web};
use tracing::error;

/// Retrieves the authenticated user's settings.
#[utoipa::path(
    tags = ["settings"],
    responses(
        (status = 200, description = "User settings.", body = UserSettings),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/user/settings")]
pub async fn user_settings_get(state: web::Data<AppState>, user: User) -> impl Responder {
    match state.api.settings(&user).get_settings().await {
        Ok(settings) => HttpResponse::Ok().json(settings),
        Err(err) => {
            error!(
                "Failed to retrieve settings for user ({}): {err:?}.",
                *user.id
            );
            generic_internal_server_error()
        }
    }
}
