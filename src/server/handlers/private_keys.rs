use crate::{
    error::Error,
    server::app_state::AppState,
    users::User,
    utils::certificates::{
        PrivateKey, PrivateKeysCreateParams, PrivateKeysExportParams, PrivateKeysUpdateParams,
    },
};
use actix_web::{HttpResponse, delete, get, post, put, web};
use utoipa::IntoParams;
use uuid::Uuid;

#[derive(serde::Deserialize, IntoParams)]
pub struct KeyIdPath {
    pub key_id: Uuid,
}

/// Lists all private keys for the authenticated user.
#[utoipa::path(
    tags = ["certificates"],
    responses(
        (status = 200, description = "List of private keys.", body = [PrivateKey]),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/certificates/private_keys")]
pub async fn private_keys_list(
    state: web::Data<AppState>,
    user: User,
) -> Result<HttpResponse, Error> {
    let keys = state.api.certificates(&user).get_private_keys().await?;
    Ok(HttpResponse::Ok().json(keys))
}

/// Gets a private key by ID.
#[utoipa::path(
    tags = ["certificates"],
    params(KeyIdPath),
    responses(
        (status = 200, description = "Private key.", body = PrivateKey),
        (status = NOT_FOUND, description = "Private key not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/certificates/private_keys/{key_id}")]
pub async fn private_keys_get(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<KeyIdPath>,
) -> Result<HttpResponse, Error> {
    let Some(key) = state
        .api
        .certificates(&user)
        .get_private_key(path.key_id)
        .await?
    else {
        return Err(Error::not_found("Private key not found."));
    };

    Ok(HttpResponse::Ok().json(key))
}

/// Creates a new private key.
#[utoipa::path(
    tags = ["certificates"],
    request_body = PrivateKeysCreateParams,
    responses(
        (status = 201, description = "Private key was successfully created.", body = PrivateKey),
        (status = BAD_REQUEST, description = "Invalid private key parameters."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/certificates/private_keys")]
pub async fn private_keys_create(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<PrivateKeysCreateParams>,
) -> Result<HttpResponse, Error> {
    let key = state
        .api
        .certificates(&user)
        .create_private_key(body.into_inner())
        .await?;
    Ok(HttpResponse::Created().json(key))
}

/// Updates an existing private key.
#[utoipa::path(
    tags = ["certificates"],
    params(KeyIdPath),
    request_body = PrivateKeysUpdateParams,
    responses(
        (status = 204, description = "Private key was successfully updated."),
        (status = NOT_FOUND, description = "Private key not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[put("/api/certificates/private_keys/{key_id}")]
pub async fn private_keys_update(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<KeyIdPath>,
    body: web::Json<PrivateKeysUpdateParams>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .certificates(&user)
        .update_private_key(path.key_id, body.into_inner())
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Deletes a private key by ID.
#[utoipa::path(
    tags = ["certificates"],
    params(KeyIdPath),
    responses(
        (status = 204, description = "Private key was successfully deleted."),
        (status = NOT_FOUND, description = "Private key not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[delete("/api/certificates/private_keys/{key_id}")]
pub async fn private_keys_delete(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<KeyIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .certificates(&user)
        .remove_private_key(path.key_id)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Exports a private key in the specified format.
#[utoipa::path(
    tags = ["certificates"],
    params(KeyIdPath),
    request_body = PrivateKeysExportParams,
    responses(
        (status = 200, description = "Exported private key data."),
        (status = NOT_FOUND, description = "Private key not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/certificates/private_keys/{key_id}/_export")]
pub async fn private_keys_export(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<KeyIdPath>,
    body: web::Json<PrivateKeysExportParams>,
) -> Result<HttpResponse, Error> {
    let data = state
        .api
        .certificates(&user)
        .export_private_key(path.key_id, body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(data))
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::schema_example,
        utils::certificates::{
            PrivateKeysCreateParams, PrivateKeysExportParams, PrivateKeysUpdateParams,
        },
    };

    #[test]
    fn private_keys_create_params_example_is_valid() {
        let example: PrivateKeysCreateParams =
            serde_json::from_value(schema_example::<PrivateKeysCreateParams>()).unwrap();
        assert!(!example.key_name.is_empty());
    }

    #[test]
    fn private_keys_update_params_example_is_valid() {
        let example: PrivateKeysUpdateParams =
            serde_json::from_value(schema_example::<PrivateKeysUpdateParams>()).unwrap();
        assert!(
            example.key_name.is_some()
                || example.new_passphrase.is_some()
                || example.passphrase.is_some()
                || example.tag_ids.is_some()
        );
    }

    #[test]
    fn private_keys_export_params_example_is_valid() {
        let _: PrivateKeysExportParams =
            serde_json::from_value(schema_example::<PrivateKeysExportParams>()).unwrap();
    }
}
