use crate::{
    error::Error,
    server::app_state::AppState,
    users::{User, UserDataExportParams, generate_export},
};
use actix_web::{HttpResponse, post, web};
use time::OffsetDateTime;

/// Exports user data as a downloadable JSON file.
#[utoipa::path(
    path = "/api/user/data/_export",
    tags = ["user-data"],
    request_body = UserDataExportParams,
    responses(
        (status = 200, description = "User data export.")
    )
)]
#[post("/_export")]
pub async fn user_data_export(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<UserDataExportParams>,
) -> Result<HttpResponse, Error> {
    let timestamp = OffsetDateTime::now_utc().unix_timestamp();
    Ok(HttpResponse::Ok()
        .insert_header((
            "Content-Disposition",
            format!("attachment; filename=\"export-{timestamp}.secutils.json\""),
        ))
        .json(generate_export(&state.api, &user, &body).await?))
}
