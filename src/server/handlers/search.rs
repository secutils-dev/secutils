use crate::{
    search::SearchFilter,
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::User,
};
use actix_web::{HttpResponse, Responder, web};
use serde::Deserialize;
use tracing::error;

#[derive(Deserialize)]
pub struct SearchParams {
    pub query: String,
}

pub async fn search(
    state: web::Data<AppState>,
    user: User,
    body_params: web::Json<SearchParams>,
) -> impl Responder {
    let search_filter = SearchFilter::default()
        .with_query(&body_params.query)
        .with_user_id(user.id);
    match state.api.search().search(search_filter) {
        Ok(search_items) => HttpResponse::Ok().json(search_items),
        Err(err) => {
            error!("Failed to perform search: {err:?}");
            generic_internal_server_error()
        }
    }
}
