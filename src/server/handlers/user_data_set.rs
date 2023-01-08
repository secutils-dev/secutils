use crate::{
    server::app_state::AppState,
    users::{User, UserDataType},
};
use actix_web::{web, HttpResponse, Responder};
use serde_derive::Deserialize;
use serde_json::json;

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

    match state
        .api
        .users()
        .set_data(&user.email, query_params.data_type, body_params.data_value)
        .await
    {
        Ok(_) => {
            log::debug!(
                "Updated data ({}) for user {}.",
                query_params.data_type.get_data_key(),
                user.handle
            );
            HttpResponse::Ok().json(json!({ "status": "ok" }))
        }
        Err(err) => {
            log::error!("Failed to update data for user {}: {:?}.", user.handle, err);
            HttpResponse::InternalServerError().json(json!({ "status": "failed" }))
        }
    }
}
