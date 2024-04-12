use crate::{
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::{User, UserData, UserDataNamespace},
};
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use time::OffsetDateTime;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDataQueryParameters {
    pub namespace: UserDataNamespace,
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
        .set_data(
            query_params.namespace,
            UserData::new(user.id, body_params.data_value, OffsetDateTime::now_utc()),
        )
        .await
    {
        log::error!("Failed to update data for user ({}): {:?}.", *user.id, err);
        return generic_internal_server_error();
    }

    log::debug!(
        "Updated data ({:?}) for the user ({}). Retrieving the latest value...",
        query_params.namespace,
        *user.id
    );

    match users_api.get_data(user.id, query_params.namespace).await {
        Ok(user_data) => HttpResponse::Ok().json(
            [(
                query_params.namespace,
                user_data.map(|user_data| user_data.value),
            )]
            .into_iter()
            .collect::<BTreeMap<_, Option<serde_json::Value>>>(),
        ),
        Err(err) => {
            log::error!(
                "Failed to retrieve data ({:?}) for user ({}): {:?}.",
                query_params.namespace,
                *user.id,
                err
            );
            generic_internal_server_error()
        }
    }
}
