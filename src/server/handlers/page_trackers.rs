use crate::{
    error::Error,
    server::{
        app_state::AppState,
        pagination::{Page, PaginationParams},
    },
    users::User,
    utils::web_scraping::{
        PageTracker, PageTrackerCreateParams, PageTrackerDebugParams, PageTrackerGetHistoryParams,
        PageTrackerUpdateParams, TrackerKind,
    },
};
use actix_web::{HttpResponse, delete, get, post, put, web};
use utoipa::IntoParams;
use uuid::Uuid;

#[derive(serde::Deserialize, IntoParams)]
pub struct TrackerIdPath {
    pub tracker_id: Uuid,
}

/// Lists page trackers for the authenticated user (paginated).
#[utoipa::path(
    tags = ["web_scraping"],
    params(PaginationParams),
    responses(
        (status = 200, description = "Paginated list of page trackers.", body = Page<PageTracker>),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/web_scraping/page_trackers")]
pub async fn page_trackers_list(
    state: web::Data<AppState>,
    user: User,
    pagination: web::Query<PaginationParams>,
) -> Result<HttpResponse, Error> {
    let trackers = state
        .api
        .web_scraping(&user)
        .list_page_trackers_page(&pagination.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(trackers))
}

/// Creates a new page tracker.
#[utoipa::path(
    tags = ["web_scraping"],
    request_body = PageTrackerCreateParams,
    responses(
        (status = 201, description = "Page tracker was successfully created.", body = PageTracker),
        (status = BAD_REQUEST, description = "Invalid page tracker parameters."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/web_scraping/page_trackers")]
pub async fn page_trackers_create(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<PageTrackerCreateParams>,
) -> Result<HttpResponse, Error> {
    let tracker = state
        .api
        .web_scraping(&user)
        .create_page_tracker(body.into_inner())
        .await?;
    Ok(HttpResponse::Created().json(tracker))
}

/// Updates an existing page tracker.
#[utoipa::path(
    tags = ["web_scraping"],
    params(TrackerIdPath),
    request_body = PageTrackerUpdateParams,
    responses(
        (status = 204, description = "Page tracker was successfully updated."),
        (status = NOT_FOUND, description = "Page tracker not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[put("/api/web_scraping/page_trackers/{tracker_id}")]
pub async fn page_trackers_update(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TrackerIdPath>,
    body: web::Json<PageTrackerUpdateParams>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .web_scraping(&user)
        .update_page_tracker(path.tracker_id, body.into_inner())
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Deletes a page tracker by ID.
#[utoipa::path(
    tags = ["web_scraping"],
    params(TrackerIdPath),
    responses(
        (status = 204, description = "Page tracker was successfully deleted."),
        (status = NOT_FOUND, description = "Page tracker not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[delete("/api/web_scraping/page_trackers/{tracker_id}")]
pub async fn page_trackers_delete(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TrackerIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .web_scraping(&user)
        .remove_page_tracker(path.tracker_id)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Returns the revision history for a page tracker.
#[utoipa::path(
    tags = ["web_scraping"],
    params(TrackerIdPath),
    request_body = PageTrackerGetHistoryParams,
    responses(
        (status = 200, description = "List of page tracker history entries."),
        (status = NOT_FOUND, description = "Page tracker not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/web_scraping/page_trackers/{tracker_id}/_history")]
pub async fn page_trackers_get_history(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TrackerIdPath>,
    body: web::Json<PageTrackerGetHistoryParams>,
) -> Result<HttpResponse, Error> {
    let history = state
        .api
        .web_scraping(&user)
        .get_page_tracker_history(path.tracker_id, body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(history))
}

/// Clears the revision history for a page tracker.
#[utoipa::path(
    tags = ["web_scraping"],
    params(TrackerIdPath),
    responses(
        (status = 204, description = "History was successfully cleared."),
        (status = NOT_FOUND, description = "Page tracker not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/web_scraping/page_trackers/{tracker_id}/_clear")]
pub async fn page_trackers_clear_history(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TrackerIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .web_scraping(&user)
        .clear_page_tracker_history(path.tracker_id)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Returns the logs for a page tracker.
#[utoipa::path(
    tags = ["web_scraping"],
    params(TrackerIdPath),
    responses(
        (status = 200, description = "List of tracker logs."),
        (status = NOT_FOUND, description = "Page tracker not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/web_scraping/page_trackers/{tracker_id}/_logs")]
pub async fn page_trackers_get_logs(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TrackerIdPath>,
) -> Result<HttpResponse, Error> {
    let logs = state
        .api
        .web_scraping(&user)
        .get_tracker_logs(path.tracker_id, TrackerKind::Page)
        .await?;
    Ok(HttpResponse::Ok().json(logs))
}

/// Clears the logs for a page tracker.
#[utoipa::path(
    tags = ["web_scraping"],
    params(TrackerIdPath),
    responses(
        (status = 204, description = "Logs were successfully cleared."),
        (status = NOT_FOUND, description = "Page tracker not found."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/web_scraping/page_trackers/{tracker_id}/_clear_logs")]
pub async fn page_trackers_clear_logs(
    state: web::Data<AppState>,
    user: User,
    path: web::Path<TrackerIdPath>,
) -> Result<HttpResponse, Error> {
    state
        .api
        .web_scraping(&user)
        .clear_tracker_logs(path.tracker_id, TrackerKind::Page)
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Returns a summary of logs across all page trackers.
#[utoipa::path(
    tags = ["web_scraping"],
    responses(
        (status = 200, description = "Logs summary for all page trackers."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/web_scraping/page_trackers/_logs_summary")]
pub async fn page_trackers_get_logs_summary(
    state: web::Data<AppState>,
    user: User,
) -> Result<HttpResponse, Error> {
    let summary = state
        .api
        .web_scraping(&user)
        .get_tracker_logs_summary(TrackerKind::Page)
        .await?;
    Ok(HttpResponse::Ok().json(summary))
}

/// Runs a page tracker in debug mode without persisting results.
#[utoipa::path(
    tags = ["web_scraping"],
    request_body = PageTrackerDebugParams,
    responses(
        (status = 200, description = "Debug result for the page tracker."),
        (status = BAD_REQUEST, description = "Invalid debug parameters."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[post("/api/web_scraping/page_trackers/_debug")]
pub async fn page_trackers_debug(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<PageTrackerDebugParams>,
) -> Result<HttpResponse, Error> {
    let result = state
        .api
        .web_scraping(&user)
        .debug_page_tracker(body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(result))
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::schema_example,
        utils::web_scraping::{
            PageTrackerCreateParams, PageTrackerDebugParams, PageTrackerGetHistoryParams,
            PageTrackerUpdateParams,
        },
    };

    #[test]
    fn page_tracker_create_params_example_is_valid() {
        let example: PageTrackerCreateParams =
            serde_json::from_value(schema_example::<PageTrackerCreateParams>()).unwrap();
        assert!(!example.name.is_empty());
    }

    #[test]
    fn page_tracker_update_params_example_is_valid() {
        let example: PageTrackerUpdateParams =
            serde_json::from_value(schema_example::<PageTrackerUpdateParams>()).unwrap();
        assert!(example.name.is_some());
        assert!(!example.name.unwrap().is_empty());
    }

    #[test]
    fn page_tracker_get_history_params_example_is_valid() {
        let _: PageTrackerGetHistoryParams =
            serde_json::from_value(schema_example::<PageTrackerGetHistoryParams>()).unwrap();
    }

    #[test]
    fn page_tracker_debug_params_example_is_valid() {
        let example: PageTrackerDebugParams =
            serde_json::from_value(schema_example::<PageTrackerDebugParams>()).unwrap();
        assert!(!example.target.extractor.is_empty());
    }
}
