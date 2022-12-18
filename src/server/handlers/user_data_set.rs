use crate::{
    server::app_state::AppState,
    users::{User, UserProfile, UserProfileData},
};
use actix_web::{web, HttpResponse, Responder};
use serde_derive::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub struct LoginParams {
    pub data: BTreeMap<String, String>,
}

pub async fn user_data_set(
    state: web::Data<AppState>,
    body_params: web::Json<LoginParams>,
    mut user: User,
) -> impl Responder {
    let body_params = body_params.into_inner();
    if body_params.data.is_empty() {
        return HttpResponse::Ok().json(json!({ "status": "ok" }));
    }

    let data = match UserProfileData::merge(
        body_params.data,
        user.profile
            .take()
            .into_iter()
            .flat_map(|mut profile| profile.data.take())
            .flat_map(|data| data.into_iter())
            .collect(),
    ) {
        Ok(data) => data,
        Err(err) => {
            log::error!(
                "Failed to validate new data for user {}: {:?}.",
                user.handle,
                err
            );
            return HttpResponse::BadRequest().json(json!({ "error": format!("{err:#}") }));
        }
    };

    let user_to_update = User {
        profile: Some(UserProfile { data: Some(data) }),
        ..user
    };

    match state.api.users().upsert(&user_to_update) {
        Ok(_) => {
            log::debug!("Updated data for user {}.", user_to_update.handle);
            HttpResponse::Ok().json(json!({ "status": "ok" }))
        }
        Err(err) => {
            log::error!(
                "Failed to update data for user {}: {:?}.",
                user_to_update.handle,
                err
            );
            HttpResponse::Unauthorized().json(json!({ "status": "failed" }))
        }
    }
}
