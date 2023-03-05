use crate::{server::app_state::AppState, users::User};
use actix_web::{web, Error, HttpResponse, Responder};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct RemoveParams {
    pub username: String,
}

pub async fn security_users_remove(
    state: web::Data<AppState>,
    body_params: web::Json<RemoveParams>,
    user: User,
) -> impl Responder {
    state.ensure_admin(&user)?;

    let body_params = body_params.into_inner();
    if body_params.username.is_empty() {
        return Ok::<HttpResponse, Error>(
            HttpResponse::BadRequest().json(json!({ "status": "failed" })),
        );
    }

    match state
        .api
        .users()
        .remove_by_email(&body_params.username)
        .await
    {
        Ok(Some(user)) => {
            log::info!("Successfully removed user: {:?}", user);
        }
        Ok(None) => {
            log::warn!("User with {} email doesn't exist.", body_params.username);
        }
        Err(err) => {
            log::error!(
                "Failed to remove user (`{}`): {:?}",
                body_params.username,
                err
            );
            return Ok(HttpResponse::InternalServerError().json(json!({ "status": "failed" })));
        }
    }

    Ok(HttpResponse::Ok().json(json!({ "status": "ok" })))
}
