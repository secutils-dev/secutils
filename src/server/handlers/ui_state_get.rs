use crate::{
    error::SecutilsError,
    server::{app_state::AppState, status::Status},
    users::{User, UserDataType, UserSettings},
    utils::Util,
};
use actix_web::{web, HttpResponse};
use anyhow::anyhow;
use serde_derive::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct License;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UiState {
    status: Status,
    license: License,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<User>,
    #[serde(skip_serializing_if = "Option::is_none")]
    settings: Option<UserSettings>,
    utils: Vec<Util>,
}

pub async fn ui_state_get(
    state: web::Data<AppState>,
    user: Option<User>,
) -> Result<HttpResponse, SecutilsError> {
    let (settings, utils) = if let Some(ref user) = user {
        (
            state
                .api
                .users()
                .get_data(user.id, UserDataType::UserSettings)
                .await?,
            state.api.utils().get_all().await?,
        )
    } else {
        (None, vec![])
    };
    Ok(HttpResponse::Ok().json(UiState {
        status: state
            .status
            .read()
            .map(|status| *status)
            .map_err(|err| anyhow!("Failed to retrieve server status: {:?}.", err))?,
        license: License,
        user,
        settings,
        utils,
    }))
}
