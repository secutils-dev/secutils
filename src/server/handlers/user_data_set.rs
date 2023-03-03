use crate::{
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::{PublicUserDataType, User, UserDataType},
};
use actix_web::{web, HttpResponse, Responder};
use serde_derive::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDataQueryParameters {
    pub data_type: PublicUserDataType,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDataBodyParameters {
    #[serde(with = "serde_bytes")]
    pub data_value: Vec<u8>,
}

pub async fn user_data_set(
    state: web::Data<AppState>,
    query_params: web::Query<SetDataQueryParameters>,
    body_params: web::Json<SetDataBodyParameters>,
    user: User,
) -> impl Responder {
    let body_params = body_params.into_inner();
    if body_params.data_value.is_empty() {
        return HttpResponse::Ok().json(json!({ "status": "ok" }));
    }

    let user_data_type = UserDataType::from(query_params.data_type);
    let users_api = state.api.users();
    if let Err(err) = users_api
        .set_data(user.id, user_data_type, body_params.data_value)
        .await
    {
        log::error!(
            "Failed to update data for user (user ID: {:?}): {:?}.",
            user.id,
            err
        );
        return generic_internal_server_error();
    }

    log::debug!(
        "Updated data ({}) for user (user ID: {:?}). Retrieving the latest value...",
        user_data_type.get_data_key(),
        user.id
    );

    match users_api.get_data(user.id, query_params.data_type).await {
        Ok(value) => HttpResponse::Ok().json(
            [(user_data_type.get_data_key().to_string(), value)]
                .into_iter()
                .collect::<BTreeMap<String, Option<serde_json::Value>>>(),
        ),
        Err(err) => {
            log::error!(
                "Failed to retrieve data ({}) for user (user ID: {:?}): {:?}.",
                user_data_type.get_data_key(),
                user.id,
                err
            );
            generic_internal_server_error()
        }
    }
}
