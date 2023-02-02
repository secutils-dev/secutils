use crate::{datastore::SearchFilter, server::app_state::AppState, users::User};
use actix_web::{web, HttpResponse, Responder};
use serde_derive::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct SearchParams {
    pub query: String,
}

pub async fn search(
    state: web::Data<AppState>,
    user: Option<User>,
    body_params: web::Json<SearchParams>,
) -> impl Responder {
    let search_filter = SearchFilter::default().with_query(&body_params.query);
    let search_filter = if let Some(user) = user {
        search_filter.with_user_id(user.id)
    } else {
        search_filter
    };

    match state.api.search().search(search_filter) {
        Ok(search_items) => HttpResponse::Ok().json(search_items),
        Err(err) => {
            log::error!("Failed to perform search: {:?}", err);
            HttpResponse::InternalServerError().json(json!({ "status": "failed" }))
        }
    }
}
