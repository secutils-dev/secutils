use crate::{error::Error, server::app_state::AppState, users::User};
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct SecretIdPath {
    pub secret_id: Uuid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSecretBody {
    pub name: String,
    pub value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSecretBody {
    pub value: String,
}

/// GET /api/user/secrets
pub async fn user_secrets_list(
    state: web::Data<AppState>,
    user: User,
) -> Result<HttpResponse, Error> {
    let secrets = state.api.secrets(&user).list_secrets().await?;
    Ok(HttpResponse::Ok().json(secrets))
}

/// POST /api/user/secrets
pub async fn user_secrets_create(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<CreateSecretBody>,
) -> Result<HttpResponse, Error> {
    let secret = state
        .api
        .secrets(&user)
        .create_secret(&body.name, &body.value)
        .await?;
    Ok(HttpResponse::Created().json(secret))
}

/// PUT /api/user/secrets/{secret_id}
pub async fn user_secrets_update(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<SecretIdPath>,
    body: web::Json<UpdateSecretBody>,
) -> Result<HttpResponse, Error> {
    let secret = state
        .api
        .secrets(&user)
        .update_secret(path.secret_id, &body.value)
        .await?;
    Ok(HttpResponse::Ok().json(secret))
}

/// DELETE /api/user/secrets/{secret_id}
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
