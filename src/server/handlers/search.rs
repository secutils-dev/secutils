use crate::{
    search::SearchFilter,
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::User,
};
use actix_web::{HttpResponse, Responder, post, web};
use serde::Deserialize;
use tracing::error;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
#[schema(example = json!({"query": "certificate"}))]
pub struct SearchParams {
    /// The search query string.
    pub query: String,
}

/// Searches across user resources.
#[utoipa::path(
    tags = ["search"],
    request_body = SearchParams,
    responses(
        (status = 200, description = "Search results."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/search")]
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

#[cfg(test)]
mod tests {
    use super::SearchParams;
    use crate::tests::schema_example;

    #[test]
    fn search_params_example_is_valid() {
        let example: SearchParams =
            serde_json::from_value(schema_example::<SearchParams>()).unwrap();
        assert!(!example.query.is_empty());
    }
}
