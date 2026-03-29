use crate::{
    error::Error,
    server::app_state::AppState,
    users::{SecretCreateParams, SecretUpdateParams, User, UserSecret},
};
use actix_web::{HttpResponse, delete, get, post, put, web};
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

#[derive(Deserialize, IntoParams)]
pub struct SecretIdPath {
    pub secret_id: Uuid,
}

/// Lists all secrets for the authenticated user (metadata only, no values).
#[utoipa::path(
    tags = ["secrets"],
    responses(
        (status = 200, description = "List of user secrets.", body = [UserSecret])
    )
)]
#[get("/api/user/secrets")]
pub async fn user_secrets_list(
    state: web::Data<AppState>,
    user: User,
) -> Result<HttpResponse, Error> {
    let secrets = state.api.secrets(&user).list_secrets().await?;
    Ok(HttpResponse::Ok().json(secrets))
}

/// Creates a new secret.
#[utoipa::path(
    tags = ["secrets"],
    request_body = SecretCreateParams,
    responses(
        (status = 201, description = "Secret was successfully created.", body = UserSecret),
        (status = BAD_REQUEST, description = "Invalid secret parameters.")
    )
)]
#[post("/api/user/secrets")]
pub async fn user_secrets_create(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<SecretCreateParams>,
) -> Result<HttpResponse, Error> {
    let secret = state
        .api
        .secrets(&user)
        .create_secret(body.into_inner())
        .await?;
    Ok(HttpResponse::Created().json(secret))
}

/// Updates an existing secret's value.
#[utoipa::path(
    tags = ["secrets"],
    params(SecretIdPath),
    request_body = SecretUpdateParams,
    responses(
        (status = 200, description = "Secret was successfully updated.", body = UserSecret),
        (status = NOT_FOUND, description = "Secret not found.")
    )
)]
#[put("/api/user/secrets/{secret_id}")]
pub async fn user_secrets_update(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<SecretIdPath>,
    body: web::Json<SecretUpdateParams>,
) -> Result<HttpResponse, Error> {
    let secret = state
        .api
        .secrets(&user)
        .update_secret(path.secret_id, body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(secret))
}

/// Deletes a secret by ID.
#[utoipa::path(
    tags = ["secrets"],
    params(SecretIdPath),
    responses(
        (status = 204, description = "Secret was successfully deleted."),
        (status = NOT_FOUND, description = "Secret not found.")
    )
)]
#[delete("/api/user/secrets/{secret_id}")]
pub async fn user_secrets_delete(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<SecretIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .secrets(&user)
        .delete_secret(path.secret_id)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}
