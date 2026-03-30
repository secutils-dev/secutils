use crate::{
    error::Error,
    server::app_state::AppState,
    users::{
        User, UserDataImportParams, UserDataImportPreviewParams, execute_import,
        generate_import_preview,
    },
};
use actix_web::{HttpResponse, post, web};

/// Previews what would be imported from the provided data.
#[utoipa::path(
    path = "/api/user/data/_import_preview",
    tags = ["user-data"],
    request_body = UserDataImportPreviewParams,
    responses(
        (status = 200, description = "Import preview result.")
    )
)]
#[post("/_import_preview")]
pub async fn user_data_import_preview(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<UserDataImportPreviewParams>,
) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok()
        .json(generate_import_preview(&state.api, &user, &body.into_inner()).await?))
}

/// Executes the data import.
#[utoipa::path(
    path = "/api/user/data/_import",
    tags = ["user-data"],
    request_body = UserDataImportParams,
    responses(
        (status = 200, description = "Import result.")
    )
)]
#[post("/_import")]
pub async fn user_data_import(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<UserDataImportParams>,
) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().json(execute_import(&state.api, &user, body.into_inner()).await?))
}
