use crate::{
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::User,
};
use actix_web::{HttpResponse, web};
use serde::Deserialize;
use tracing::error;

#[derive(Deserialize)]
pub struct SecretNamePath {
    pub secret_name: String,
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
pub async fn user_secrets_list(state: web::Data<AppState>, user: User) -> HttpResponse {
    match state.api.secrets(&user).list_secrets().await {
        Ok(secrets) => HttpResponse::Ok().json(secrets),
        Err(err) => {
            error!(user.id = %user.id, "Failed to list secrets: {err:?}");
            generic_internal_server_error()
        }
    }
}

/// POST /api/user/secrets
pub async fn user_secrets_create(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<CreateSecretBody>,
) -> HttpResponse {
    match state
        .api
        .secrets(&user)
        .create_secret(&body.name, &body.value)
        .await
    {
        Ok(secret) => HttpResponse::Created().json(secret),
        Err(err) => {
            let err_string = format!("{err:?}");
            if err_string.contains("unique constraint") || err_string.contains("duplicate key") {
                HttpResponse::Conflict().json(serde_json::json!({
                    "message": format!("A secret with name '{}' already exists.", body.name)
                }))
            } else if is_client_error(&err_string) {
                HttpResponse::BadRequest().json(serde_json::json!({ "message": err.to_string() }))
            } else {
                error!(user.id = %user.id, "Failed to create secret: {err:?}");
                generic_internal_server_error()
            }
        }
    }
}

/// PUT /api/user/secrets/{secret_name}
pub async fn user_secrets_update(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<SecretNamePath>,
    body: web::Json<UpdateSecretBody>,
) -> HttpResponse {
    match state
        .api
        .secrets(&user)
        .update_secret(&path.secret_name, &body.value)
        .await
    {
        Ok(secret) => HttpResponse::Ok().json(secret),
        Err(err) => {
            let err_string = err.to_string();
            if err_string.contains("not found") {
                HttpResponse::NotFound().json(serde_json::json!({
                    "message": format!("Secret '{}' not found.", path.secret_name)
                }))
            } else if is_client_error(&err_string) {
                HttpResponse::BadRequest().json(serde_json::json!({ "message": err_string }))
            } else {
                error!(user.id = %user.id, "Failed to update secret '{}': {err:?}", path.secret_name);
                generic_internal_server_error()
            }
        }
    }
}

/// DELETE /api/user/secrets/{secret_name}
pub async fn user_secrets_delete(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<SecretNamePath>,
) -> HttpResponse {
    match state
        .api
        .secrets(&user)
        .delete_secret(&path.secret_name)
        .await
    {
        Ok(_) => HttpResponse::NoContent().finish(),
        Err(err) => {
            let err_string = err.to_string();
            if err_string.contains("not found") {
                HttpResponse::NotFound().json(serde_json::json!({
                    "message": format!("Secret '{}' not found.", path.secret_name)
                }))
            } else {
                error!(user.id = %user.id, "Failed to delete secret '{}': {err:?}", path.secret_name);
                generic_internal_server_error()
            }
        }
    }
}

fn is_client_error(msg: &str) -> bool {
    msg.contains("Secret name must")
        || msg.contains("Secret value cannot")
        || msg.contains("Secret value must")
        || msg.contains("Maximum number of secrets")
}
