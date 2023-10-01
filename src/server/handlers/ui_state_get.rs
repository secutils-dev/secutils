use crate::{
    error::SecutilsError,
    server::{status::Status, AppState},
    users::{ClientUserShare, PublicUserDataNamespace, User, UserSettings, UserShare},
    utils::Util,
};
use actix_web::{web, HttpResponse};
use anyhow::anyhow;
use serde::Serialize;
use std::ops::Deref;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct License;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UiState<'a> {
    status: &'a Status,
    license: License,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<User>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_share: Option<ClientUserShare>,
    #[serde(skip_serializing_if = "Option::is_none")]
    settings: Option<UserSettings>,
    utils: Vec<Util>,
}

pub async fn ui_state_get(
    state: web::Data<AppState>,
    user: Option<User>,
    user_share: Option<UserShare>,
) -> Result<HttpResponse, SecutilsError> {
    // Settings only available for authenticated users.
    let settings = if let Some(ref user) = user {
        state
            .api
            .users()
            .get_data(user.id, PublicUserDataNamespace::UserSettings)
            .await?
            .map(|user_data| user_data.value)
    } else {
        None
    };

    // Utils are only available for authenticated users or when accessing shared resources.
    let utils = if user.is_some() || user_share.is_some() {
        state.api.utils().get_all().await?
    } else {
        vec![]
    };

    Ok(HttpResponse::Ok().json(UiState {
        status: state
            .status
            .read()
            .map_err(|err| anyhow!("Failed to retrieve server status: {:?}.", err))?
            .deref(),
        license: License,
        user,
        user_share: user_share.map(ClientUserShare::from),
        settings,
        utils,
    }))
}
