use crate::{
    error::Error,
    security::{Credentials, Operator},
    server::app_state::AppState,
    users::{
        ApiKeyCreateParams, ApiKeyCreateResponse, ApiKeyRegenerateParams, ApiKeyUpdateParams, User,
        UserApiKey, UserId,
    },
};
use actix_web::{HttpResponse, delete, get, post, put, web};
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

#[derive(Deserialize, IntoParams)]
pub struct ApiKeyIdPath {
    pub api_key_id: Uuid,
}

#[derive(Deserialize, IntoParams)]
pub struct UserIdPath {
    pub user_id: Uuid,
}

/// Rejects requests authenticated with API keys (API keys cannot manage API keys).
fn reject_api_key_credentials(credentials: &Credentials) -> Result<(), Error> {
    if matches!(credentials, Credentials::ApiKey(_)) {
        Err(Error::access_forbidden())
    } else {
        Ok(())
    }
}

/// Lists all API keys for the authenticated user.
#[utoipa::path(
    tags = ["api_keys"],
    responses(
        (status = 200, description = "List of user API keys.", body = [UserApiKey]),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials."),
        (status = FORBIDDEN, description = "API keys cannot manage API keys.")
    )
)]
#[get("/api/user/api_keys")]
pub async fn user_api_keys_list(
    state: web::Data<AppState>,
    user: User,
    credentials: Credentials,
) -> Result<HttpResponse, Error> {
    reject_api_key_credentials(&credentials)?;
    let keys = state.api.api_keys(&user).list_api_keys().await?;
    Ok(HttpResponse::Ok().json(keys))
}

/// Creates a new API key. The plaintext token is included in the response and
/// cannot be retrieved again.
#[utoipa::path(
    tags = ["api_keys"],
    request_body = ApiKeyCreateParams,
    responses(
        (status = 201, description = "API key was successfully created.", body = ApiKeyCreateResponse),
        (status = BAD_REQUEST, description = "Invalid API key parameters."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials."),
        (status = FORBIDDEN, description = "API keys cannot manage API keys.")
    )
)]
#[post("/api/user/api_keys")]
pub async fn user_api_keys_create(
    state: web::Data<AppState>,
    user: User,
    credentials: Credentials,
    body: web::Json<ApiKeyCreateParams>,
) -> Result<HttpResponse, Error> {
    reject_api_key_credentials(&credentials)?;
    let (api_key, token) = state
        .api
        .api_keys(&user)
        .create_api_key(body.into_inner())
        .await?;
    Ok(HttpResponse::Created().json(ApiKeyCreateResponse { api_key, token }))
}

/// Updates an existing API key's name.
#[utoipa::path(
    tags = ["api_keys"],
    params(ApiKeyIdPath),
    request_body = ApiKeyUpdateParams,
    responses(
        (status = 200, description = "API key was successfully updated.", body = UserApiKey),
        (status = NOT_FOUND, description = "API key not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials."),
        (status = FORBIDDEN, description = "API keys cannot manage API keys.")
    )
)]
#[put("/api/user/api_keys/{api_key_id}")]
pub async fn user_api_keys_update(
    state: web::Data<AppState>,
    user: User,
    credentials: Credentials,
    path: web::Path<ApiKeyIdPath>,
    body: web::Json<ApiKeyUpdateParams>,
) -> Result<HttpResponse, Error> {
    reject_api_key_credentials(&credentials)?;
    let api_key = state
        .api
        .api_keys(&user)
        .update_api_key(path.api_key_id, body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(api_key))
}

/// Deletes an API key by ID.
#[utoipa::path(
    tags = ["api_keys"],
    params(ApiKeyIdPath),
    responses(
        (status = 204, description = "API key was successfully deleted."),
        (status = NOT_FOUND, description = "API key not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials."),
        (status = FORBIDDEN, description = "API keys cannot manage API keys.")
    )
)]
#[delete("/api/user/api_keys/{api_key_id}")]
pub async fn user_api_keys_delete(
    state: web::Data<AppState>,
    user: User,
    credentials: Credentials,
    path: web::Path<ApiKeyIdPath>,
) -> Result<HttpResponse, Error> {
    reject_api_key_credentials(&credentials)?;
    state
        .api
        .api_keys(&user)
        .delete_api_key(path.api_key_id)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Regenerates the token for an existing API key. The old token is immediately
/// invalidated and the new plaintext token is returned (shown once).
#[utoipa::path(
    tags = ["api_keys"],
    params(ApiKeyIdPath),
    request_body = ApiKeyRegenerateParams,
    responses(
        (status = 200, description = "API key was successfully regenerated.", body = ApiKeyCreateResponse),
        (status = NOT_FOUND, description = "API key not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials."),
        (status = FORBIDDEN, description = "API keys cannot manage API keys.")
    )
)]
#[post("/api/user/api_keys/{api_key_id}/_regenerate")]
pub async fn user_api_keys_regenerate(
    state: web::Data<AppState>,
    user: User,
    credentials: Credentials,
    path: web::Path<ApiKeyIdPath>,
    body: web::Json<ApiKeyRegenerateParams>,
) -> Result<HttpResponse, Error> {
    reject_api_key_credentials(&credentials)?;
    let (api_key, token) = state
        .api
        .api_keys(&user)
        .regenerate_api_key(path.api_key_id, body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(ApiKeyCreateResponse { api_key, token }))
}

/// Creates an API key for a specific user (operator-only provisioning endpoint).
#[utoipa::path(
    tags = ["api_keys"],
    params(UserIdPath),
    request_body = ApiKeyCreateParams,
    responses(
        (status = 201, description = "API key was successfully created for the user.", body = ApiKeyCreateResponse),
        (status = BAD_REQUEST, description = "Invalid API key parameters."),
        (status = UNAUTHORIZED, description = "Missing or invalid operator credentials."),
        (status = NOT_FOUND, description = "User not found.")
    )
)]
#[post("/api/users/{user_id}/api_keys")]
pub async fn user_api_keys_create_for_user(
    state: web::Data<AppState>,
    _operator: Operator,
    path: web::Path<UserIdPath>,
    body: web::Json<ApiKeyCreateParams>,
) -> Result<HttpResponse, Error> {
    let user_id = UserId::from(path.user_id);
    let user = state
        .api
        .users()
        .get(user_id)
        .await?
        .ok_or_else(|| Error::not_found(format!("User '{}' not found.", path.user_id)))?;

    let (api_key, token) = state
        .api
        .api_keys(&user)
        .create_api_key(body.into_inner())
        .await?;
    Ok(HttpResponse::Created().json(ApiKeyCreateResponse { api_key, token }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::schema_example;

    #[test]
    fn api_key_create_params_example_is_valid() {
        let example: ApiKeyCreateParams =
            serde_json::from_value(schema_example::<ApiKeyCreateParams>()).unwrap();
        assert!(!example.name.is_empty());
    }

    #[test]
    fn api_key_update_params_example_is_valid() {
        let example: ApiKeyUpdateParams =
            serde_json::from_value(schema_example::<ApiKeyUpdateParams>()).unwrap();
        assert!(!example.name.is_empty());
    }

    #[test]
    fn api_key_regenerate_params_example_is_valid() {
        let _example: ApiKeyRegenerateParams =
            serde_json::from_value(schema_example::<ApiKeyRegenerateParams>()).unwrap();
    }
}
