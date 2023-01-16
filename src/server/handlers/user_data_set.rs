use crate::{
    server::app_state::AppState,
    users::{User, UserDataType},
};
use actix_web::{web, HttpResponse, Responder};
use serde_derive::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDataQueryParameters {
    pub data_type: UserDataType,
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

    let users_api = state.api.users();

    if let Err(err) = users_api
        .set_data(user.id, query_params.data_type, body_params.data_value)
        .await
    {
        log::error!("Failed to update data for user {}: {:?}.", user.handle, err);
        return HttpResponse::InternalServerError().json(json!({ "status": "failed" }));
    }

    log::debug!(
        "Updated data ({}) for user {}. Retrieving the latest value...",
        query_params.data_type.get_data_key(),
        user.handle
    );

    match users_api.get_data(user.id, query_params.data_type).await {
        Ok(value) => HttpResponse::Ok().json(
            [(query_params.data_type.get_data_key().to_string(), value)]
                .into_iter()
                .collect::<BTreeMap<String, Option<serde_json::Value>>>(),
        ),
        Err(err) => {
            log::error!(
                "Failed to retrieve data ({}) for user {}: {:?}.",
                query_params.data_type.get_data_key(),
                user.handle,
                err
            );
            HttpResponse::InternalServerError().json(json!({ "status": "failed" }))
        }
    }
}
