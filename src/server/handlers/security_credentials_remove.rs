use crate::{
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::User,
};
use actix_web::{web, HttpResponse, Responder};
use serde_derive::Deserialize;
use serde_json::json;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PathCredentialsType {
    Password,
    Passkey,
}

#[derive(Deserialize)]
pub struct PathParams {
    pub credentials: PathCredentialsType,
}

pub async fn security_credentials_remove(
    state: web::Data<AppState>,
    path_params: web::Path<PathParams>,
    mut user: User,
) -> impl Responder {
    // Make sure that user doesn't delete ALL their credentials.
    match path_params.credentials {
        PathCredentialsType::Password if user.credentials.passkey.is_none() => {
            log::error!(
                "Cannot remove password credentials when passkey is not set (user ID: {:?}).",
                user.id
            );
            return HttpResponse::BadRequest().json(json!({
                "message": "Cannot remove password credentials when passkey is not set."
            }));
        }
        PathCredentialsType::Passkey if user.credentials.password_hash.is_none() => {
            log::error!(
                "Cannot remove passkey credentials when password is not set (user ID: {:?}).",
                user.id
            );
            return HttpResponse::BadRequest().json(json!({
                "message": "Cannot remove passkey credentials when password is not set."
            }));
        }
        PathCredentialsType::Password => {
            user.credentials.password_hash.take();
        }
        PathCredentialsType::Passkey => {
            user.credentials.passkey.take();
        }
    }

    let users_api = state.api.users();
    match users_api.upsert(&user).await {
        Ok(_) => {
            log::info!(
                "Successfully removed {:?} credentials (user ID: {:?}).",
                path_params.credentials,
                user.id
            );
            HttpResponse::NoContent().finish()
        }
        Err(err) => {
            log::error!(
                "Failed to remove {:?} credentials (user ID: {:?}): {:?}",
                path_params.credentials,
                user.id,
                err
            );
            generic_internal_server_error()
        }
    }
}
