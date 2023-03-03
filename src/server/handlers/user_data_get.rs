use crate::{
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::{PublicUserDataType, User},
};
use actix_web::{web, HttpResponse, Responder};
use serde_derive::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDataQueryParameters {
    pub data_type: PublicUserDataType,
}

pub async fn user_data_get(
    state: web::Data<AppState>,
    query_params: web::Query<GetDataQueryParameters>,
    user: User,
) -> impl Responder {
    let users_api = state.api.users();
    match users_api.get_data(user.id, query_params.data_type).await {
        Ok(value) => HttpResponse::Ok().json(
            [(query_params.data_type, value)]
                .into_iter()
                .collect::<BTreeMap<_, Option<serde_json::Value>>>(),
        ),
        Err(err) => {
            log::error!(
                "Failed to retrieve data ({:?}) for user (user ID: {:?}): {:?}.",
                query_params.data_type,
                user.id,
                err
            );
            generic_internal_server_error()
        }
    }
}
