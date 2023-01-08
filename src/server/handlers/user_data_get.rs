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
pub struct GetDataQueryParameters {
    pub data_type: UserDataType,
}

pub async fn user_data_get(
    state: web::Data<AppState>,
    query_params: web::Query<GetDataQueryParameters>,
    user: User,
) -> impl Responder {
    match state
        .api
        .users()
        .get_data(&user.email, query_params.data_type)
        .await
    {
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
