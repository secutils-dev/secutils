use crate::{
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::{User, UserSettings, UserSettingsSetter},
};
use actix_web::{HttpResponse, Responder, post, web};
use tracing::error;

/// Updates user settings. Keys map to new values, or null to remove a setting.
#[utoipa::path(
    tags = ["settings"],
    request_body = UserSettingsSetter,
    responses(
        (status = 200, description = "Updated user settings.", body = UserSettings)
    )
)]
#[post("/api/user/settings")]
pub async fn user_settings_set(
    state: web::Data<AppState>,
    body_params: web::Json<UserSettingsSetter>,
    user: User,
) -> impl Responder {
    let setter = body_params.into_inner();

    let settings = state.api.settings(&user);
    if setter.0.is_empty() {
        return HttpResponse::Ok().json(settings.get_settings().await.ok().flatten());
    }

    match settings.set_settings(setter).await {
        Ok(settings) => HttpResponse::Ok().json(settings),
        Err(err) => {
            error!(
                "Failed to update settings for user ({}): {err:?}.",
                *user.id
            );
            generic_internal_server_error()
        }
    }
}
