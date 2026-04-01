use crate::{
    error::Error,
    server::app_state::AppState,
    users::User,
    utils::webhooks::{Responder, ResponderStats, RespondersCreateParams, RespondersUpdateParams},
};
use actix_web::{HttpResponse, delete, get, post, put, web};
use utoipa::IntoParams;
use uuid::Uuid;

#[derive(serde::Deserialize, IntoParams)]
pub struct ResponderIdPath {
    pub responder_id: Uuid,
}

/// Lists all responders for the authenticated user.
#[utoipa::path(
    tags = ["webhooks"],
    responses(
        (status = 200, description = "List of responders.", body = [Responder]),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/webhooks/responders")]
pub async fn responders_list(
    state: web::Data<AppState>,
    user: User,
) -> Result<HttpResponse, Error> {
    let responders = state.api.webhooks(&user).get_responders().await?;
    Ok(HttpResponse::Ok().json(responders))
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

/// Returns aggregate stats for all responders.
#[utoipa::path(
    tags = ["webhooks"],
    responses(
        (status = 200, description = "List of responder stats.", body = [ResponderStats]),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/webhooks/responders/_stats")]
pub async fn responders_get_stats(
    state: web::Data<AppState>,
    user: User,
) -> Result<HttpResponse, Error> {
    let stats = state.api.webhooks(&user).get_responders_stats().await?;
    Ok(HttpResponse::Ok().json(stats))
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::schema_example,
        utils::webhooks::{RespondersCreateParams, RespondersUpdateParams},
    };

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
