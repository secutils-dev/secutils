use crate::server::app_state::AppState;
use actix_web::{web, HttpResponse, Responder};
use serde_derive::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct SearchParams {
    pub query: String,
}

pub async fn search(
    state: web::Data<AppState>,
    body_params: web::Json<SearchParams>,
) -> impl Responder {
    let body_params = body_params.into_inner();
    match state.api.search().search(body_params.query) {
        Ok(search_items) => HttpResponse::Ok().json(search_items),
        Err(err) => {
            log::error!("Failed to perform search: {:?}", err);
            HttpResponse::InternalServerError().json(json!({ "status": "failed" }))
        }
    }
}
