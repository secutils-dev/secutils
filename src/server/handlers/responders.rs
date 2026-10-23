use crate::{
    error::Error,
    server::{
        app_state::AppState,
        pagination::{Page, PaginationParams},
    },
    users::User,
    utils::webhooks::{
        Responder, ResponderStorageStats, ResponderSummary, RespondersClearStorageParams,
        RespondersCreateParams, RespondersUpdateParams,
    },
};
use actix_web::{HttpResponse, delete, get, post, put, web};
use serde::Serialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(serde::Deserialize, IntoParams)]
pub struct ResponderIdPath {
    pub responder_id: Uuid,
}

/// Lists responder summaries for the authenticated user (paginated). Summaries omit the body and
/// script settings; use the responder detail endpoint to get them.
#[utoipa::path(
    tags = ["webhooks"],
    params(PaginationParams),
    responses(
        (status = 200, description = "Paginated list of responder summaries.", body = Page<ResponderSummary>),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/webhooks/responders")]
pub async fn responders_list(
    state: web::Data<AppState>,
    user: User,
    pagination: web::Query<PaginationParams>,
) -> Result<HttpResponse, Error> {
    let responders = state
        .api
        .webhooks(&user)
        .list_responders_page(&pagination.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(responders))
}

/// Returns a responder by ID, including all of its settings.
#[utoipa::path(
    tags = ["webhooks"],
    params(ResponderIdPath),
    responses(
        (status = 200, description = "Responder.", body = Responder),
        (status = NOT_FOUND, description = "Responder not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/webhooks/responders/{responder_id}")]
pub async fn responders_get(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<ResponderIdPath>,
) -> Result<HttpResponse, Error> {
    match state
        .api
        .webhooks(&user)
        .get_responder(path.responder_id)
        .await?
    {
        Some(responder) => Ok(HttpResponse::Ok().json(responder)),
        None => Ok(HttpResponse::NotFound().finish()),
    }
}

/// Creates a new responder.
#[utoipa::path(
    tags = ["webhooks"],
    request_body = RespondersCreateParams,
    responses(
        (status = 201, description = "Responder was successfully created.", body = Responder),
        (status = BAD_REQUEST, description = "Invalid responder parameters."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/webhooks/responders")]
pub async fn responders_create(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<RespondersCreateParams>,
) -> Result<HttpResponse, Error> {
    let responder = state
        .api
        .webhooks(&user)
        .create_responder(body.into_inner())
        .await?;
    Ok(HttpResponse::Created().json(responder))
}

/// Updates an existing responder.
#[utoipa::path(
    tags = ["webhooks"],
    params(ResponderIdPath),
    request_body = RespondersUpdateParams,
    responses(
        (status = 204, description = "Responder was successfully updated."),
        (status = NOT_FOUND, description = "Responder not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[put("/api/webhooks/responders/{responder_id}")]
pub async fn responders_update(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<ResponderIdPath>,
    body: web::Json<RespondersUpdateParams>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .webhooks(&user)
        .update_responder(path.responder_id, body.into_inner())
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Deletes a responder by ID.
#[utoipa::path(
    tags = ["webhooks"],
    params(ResponderIdPath),
    responses(
        (status = 204, description = "Responder was successfully deleted."),
        (status = NOT_FOUND, description = "Responder not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[delete("/api/webhooks/responders/{responder_id}")]
pub async fn responders_delete(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<ResponderIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .webhooks(&user)
        .remove_responder(path.responder_id)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Returns the captured request history for a responder.
#[utoipa::path(
    tags = ["webhooks"],
    params(ResponderIdPath),
    responses(
        (status = 200, description = "List of captured requests."),
        (status = NOT_FOUND, description = "Responder not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/webhooks/responders/{responder_id}/_history")]
pub async fn responders_get_history(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<ResponderIdPath>,
) -> Result<HttpResponse, Error> {
    let requests = state
        .api
        .webhooks(&user)
        .get_responder_requests(path.responder_id)
        .await?;
    Ok(HttpResponse::Ok().json(requests))
}

/// Clears the captured request history for a responder.
#[utoipa::path(
    tags = ["webhooks"],
    params(ResponderIdPath),
    responses(
        (status = 204, description = "History was successfully cleared."),
        (status = NOT_FOUND, description = "Responder not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/webhooks/responders/{responder_id}/_clear")]
pub async fn responders_clear_history(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<ResponderIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .webhooks(&user)
        .clear_responder_requests(path.responder_id)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Result of clearing a responder's script key-value storage.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RespondersClearStorageResponse {
    /// Number of removed storage entries.
    pub removed: u64,
}

/// Returns usage of a responder's script key-value storage (`secutils.kv.*`) and its quotas.
#[utoipa::path(
    tags = ["webhooks"],
    params(ResponderIdPath),
    responses(
        (status = 200, description = "Responder storage usage.", body = ResponderStorageStats),
        (status = NOT_FOUND, description = "Responder not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/webhooks/responders/{responder_id}/_storage")]
pub async fn responders_get_storage(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<ResponderIdPath>,
) -> Result<HttpResponse, Error> {
    let stats = state
        .api
        .webhooks(&user)
        .get_responder_storage(path.responder_id)
        .await?;
    Ok(HttpResponse::Ok().json(stats))
}

/// Clears a responder's script key-value storage, entirely or only entries under a key prefix.
#[utoipa::path(
    tags = ["webhooks"],
    params(ResponderIdPath),
    request_body = RespondersClearStorageParams,
    responses(
        (status = 200, description = "Storage was successfully cleared.", body = RespondersClearStorageResponse),
        (status = BAD_REQUEST, description = "Invalid key prefix."),
        (status = NOT_FOUND, description = "Responder not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/webhooks/responders/{responder_id}/_clear_storage")]
pub async fn responders_clear_storage(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<ResponderIdPath>,
    body: web::Json<RespondersClearStorageParams>,
) -> Result<HttpResponse, Error> {
    let removed = state
        .api
        .webhooks(&user)
        .clear_responder_storage(path.responder_id, body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(RespondersClearStorageResponse { removed }))
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::schema_example,
        utils::webhooks::{
            RespondersClearStorageParams, RespondersCreateParams, RespondersUpdateParams,
        },
    };

    #[test]
    fn responders_clear_storage_params_example_is_valid() {
        let example: RespondersClearStorageParams =
            serde_json::from_value(schema_example::<RespondersClearStorageParams>()).unwrap();
        assert_eq!(example.prefix.as_deref(), Some("req/"));
    }

    #[test]
    fn responders_create_params_example_is_valid() {
        let example: RespondersCreateParams =
            serde_json::from_value(schema_example::<RespondersCreateParams>()).unwrap();
        assert!(!example.name.is_empty());
    }

    #[test]
    fn responders_update_params_example_is_valid() {
        let _: RespondersUpdateParams =
            serde_json::from_value(schema_example::<RespondersUpdateParams>()).unwrap();
    }
}
