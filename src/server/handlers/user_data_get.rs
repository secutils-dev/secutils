use crate::{
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::{PublicUserDataNamespace, User},
};
use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDataQueryParameters {
    pub namespace: PublicUserDataNamespace,
}

pub async fn user_data_get(
    state: web::Data<AppState>,
    query_params: web::Query<GetDataQueryParameters>,
    user: User,
) -> impl Responder {
    let users_api = state.api.users();
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
