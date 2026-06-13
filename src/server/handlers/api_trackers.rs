use crate::{
    error::Error,
    server::{
        app_state::AppState,
        pagination::{Page, PaginationParams},
    },
    users::User,
    utils::web_scraping::{
        ApiTracker, ApiTrackerCreateParams, ApiTrackerDebugParams, ApiTrackerGetHistoryParams,
        ApiTrackerTestParams, ApiTrackerTestResult, ApiTrackerUpdateParams, TrackerKind,
    },
};
use actix_web::{HttpResponse, delete, get, post, put, web};
use utoipa::IntoParams;
use uuid::Uuid;

#[derive(serde::Deserialize, IntoParams)]
pub struct TrackerIdPath {
    pub tracker_id: Uuid,
}

/// Lists API trackers for the authenticated user (paginated).
#[utoipa::path(
    tags = ["web_scraping"],
    params(PaginationParams),
    responses(
        (status = 200, description = "Paginated list of API trackers.", body = Page<ApiTracker>),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/web_scraping/api_trackers")]
pub async fn api_trackers_list(
    state: web::Data<AppState>,
    user: User,
    pagination: web::Query<PaginationParams>,
) -> Result<HttpResponse, Error> {
    let trackers = state
        .api
        .web_scraping(&user)
        .list_api_trackers_page(&pagination.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(trackers))
}

/// Creates a new API tracker.
#[utoipa::path(
    tags = ["web_scraping"],
    request_body = ApiTrackerCreateParams,
    responses(
        (status = 201, description = "API tracker was successfully created.", body = ApiTracker),
        (status = BAD_REQUEST, description = "Invalid API tracker parameters."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/web_scraping/api_trackers")]
pub async fn api_trackers_create(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<ApiTrackerCreateParams>,
) -> Result<HttpResponse, Error> {
    let tracker = state
        .api
        .web_scraping(&user)
        .create_api_tracker(body.into_inner())
        .await?;
    Ok(HttpResponse::Created().json(tracker))
}

/// Updates an existing API tracker.
#[utoipa::path(
    tags = ["web_scraping"],
    params(TrackerIdPath),
    request_body = ApiTrackerUpdateParams,
    responses(
        (status = 204, description = "API tracker was successfully updated."),
        (status = NOT_FOUND, description = "API tracker not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[put("/api/web_scraping/api_trackers/{tracker_id}")]
pub async fn api_trackers_update(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TrackerIdPath>,
    body: web::Json<ApiTrackerUpdateParams>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .web_scraping(&user)
        .update_api_tracker(path.tracker_id, body.into_inner())
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Deletes an API tracker by ID.
#[utoipa::path(
    tags = ["web_scraping"],
    params(TrackerIdPath),
    responses(
        (status = 204, description = "API tracker was successfully deleted."),
        (status = NOT_FOUND, description = "API tracker not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[delete("/api/web_scraping/api_trackers/{tracker_id}")]
pub async fn api_trackers_delete(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TrackerIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .web_scraping(&user)
        .remove_api_tracker(path.tracker_id)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Returns the revision history for an API tracker.
#[utoipa::path(
    tags = ["web_scraping"],
    params(TrackerIdPath),
    request_body = ApiTrackerGetHistoryParams,
    responses(
        (status = 200, description = "List of API tracker revisions."),
        (status = NOT_FOUND, description = "API tracker not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/web_scraping/api_trackers/{tracker_id}/_history")]
pub async fn api_trackers_get_history(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TrackerIdPath>,
    body: web::Json<ApiTrackerGetHistoryParams>,
) -> Result<HttpResponse, Error> {
    let history = state
        .api
        .web_scraping(&user)
        .get_api_tracker_history(path.tracker_id, body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(history))
}

/// Clears the revision history for an API tracker.
#[utoipa::path(
    tags = ["web_scraping"],
    params(TrackerIdPath),
    responses(
        (status = 204, description = "History was successfully cleared."),
        (status = NOT_FOUND, description = "API tracker not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/web_scraping/api_trackers/{tracker_id}/_clear")]
pub async fn api_trackers_clear_history(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TrackerIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .web_scraping(&user)
        .clear_api_tracker_history(path.tracker_id)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Returns execution logs for a specific API tracker.
#[utoipa::path(
    tags = ["web_scraping"],
    params(TrackerIdPath),
    responses(
        (status = 200, description = "List of tracker execution logs."),
        (status = NOT_FOUND, description = "API tracker not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/web_scraping/api_trackers/{tracker_id}/_logs")]
pub async fn api_trackers_get_logs(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TrackerIdPath>,
) -> Result<HttpResponse, Error> {
    let logs = state
        .api
        .web_scraping(&user)
        .get_tracker_logs(path.tracker_id, TrackerKind::Api)
        .await?;
    Ok(HttpResponse::Ok().json(logs))
}

/// Clears execution logs for a specific API tracker.
#[utoipa::path(
    tags = ["web_scraping"],
    params(TrackerIdPath),
    responses(
        (status = 204, description = "Logs were successfully cleared."),
        (status = NOT_FOUND, description = "API tracker not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/web_scraping/api_trackers/{tracker_id}/_clear_logs")]
pub async fn api_trackers_clear_logs(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TrackerIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .web_scraping(&user)
        .clear_tracker_logs(path.tracker_id, TrackerKind::Api)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Returns a summary of recent execution logs for all API trackers.
#[utoipa::path(
    tags = ["web_scraping"],
    responses(
        (status = 200, description = "Logs summary keyed by tracker ID."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/web_scraping/api_trackers/_logs_summary")]
pub async fn api_trackers_get_logs_summary(
    state: web::Data<AppState>,
    user: User,
) -> Result<HttpResponse, Error> {
    let summary = state
        .api
        .web_scraping(&user)
        .get_tracker_logs_summary(TrackerKind::Api)
        .await?;
    Ok(HttpResponse::Ok().json(summary))
}

/// Sends a test HTTP request using the provided API tracker target configuration.
#[utoipa::path(
    tags = ["web_scraping"],
    request_body = ApiTrackerTestParams,
    responses(
        (status = 200, description = "Test request result.", body = ApiTrackerTestResult),
        (status = BAD_REQUEST, description = "Invalid test parameters."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/web_scraping/api_trackers/_test")]
pub async fn api_trackers_test(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<ApiTrackerTestParams>,
) -> Result<HttpResponse, Error> {
    let result = state
        .api
        .web_scraping(&user)
        .test_api_request(body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(result))
}

/// Runs the full debug pipeline for an API tracker without persisting anything.
#[utoipa::path(
    tags = ["web_scraping"],
    request_body = ApiTrackerDebugParams,
    responses(
        (status = 200, description = "Debug result."),
        (status = BAD_REQUEST, description = "Invalid debug parameters."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/web_scraping/api_trackers/_debug")]
pub async fn api_trackers_debug(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<ApiTrackerDebugParams>,
) -> Result<HttpResponse, Error> {
    let result = state
        .api
        .web_scraping(&user)
        .debug_api_tracker(body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(result))
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::schema_example,
        utils::web_scraping::{
            ApiTrackerCreateParams, ApiTrackerDebugParams, ApiTrackerGetHistoryParams,
            ApiTrackerTestParams, ApiTrackerUpdateParams,
        },
    };

    #[test]
    fn api_tracker_create_params_example_is_valid() {
        let example: ApiTrackerCreateParams =
            serde_json::from_value(schema_example::<ApiTrackerCreateParams>()).unwrap();
        assert!(!example.name.is_empty());
    }

    #[test]
    fn api_tracker_update_params_example_is_valid() {
        let example: ApiTrackerUpdateParams =
            serde_json::from_value(schema_example::<ApiTrackerUpdateParams>()).unwrap();
        assert!(example.name.is_some());
        assert!(!example.name.unwrap().is_empty());
    }

    #[test]
    fn api_tracker_get_history_params_example_is_valid() {
        let _: ApiTrackerGetHistoryParams =
            serde_json::from_value(schema_example::<ApiTrackerGetHistoryParams>()).unwrap();
    }

    #[test]
    fn api_tracker_test_params_example_is_valid() {
        let example: ApiTrackerTestParams =
            serde_json::from_value(schema_example::<ApiTrackerTestParams>()).unwrap();
        assert!(!example.target.url.as_str().is_empty());
    }

    #[test]
    fn api_tracker_debug_params_example_is_valid() {
        let example: ApiTrackerDebugParams =
            serde_json::from_value(schema_example::<ApiTrackerDebugParams>()).unwrap();
        assert!(!example.target.url.as_str().is_empty());
    }
}
