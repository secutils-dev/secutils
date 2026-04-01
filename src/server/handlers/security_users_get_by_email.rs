use crate::{
    security::Operator,
    server::{AppState, http_errors::generic_internal_server_error},
};
use actix_web::{Error, HttpResponse, Responder, get, web};
use serde::Deserialize;
use tracing::error;
use utoipa::IntoParams;

#[derive(Deserialize, IntoParams)]
pub struct GetByEmailQuery {
    /// The email address to look up.
    email: String,
}

/// Retrieves a user by email (operator-only).
#[utoipa::path(
    tags = ["users"],
    params(GetByEmailQuery),
    responses(
        (status = 200, description = "The requested user."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials."),
        (status = FORBIDDEN, description = "Caller is not an operator."),
        (status = NOT_FOUND, description = "User not found.")
    )
)]
#[get("/api/users")]
pub async fn security_users_get_by_email(
    state: web::Data<AppState>,
    operator: Operator,
    query: web::Query<GetByEmailQuery>,
) -> impl Responder {
    Ok::<HttpResponse, Error>(match state.api.users().get_by_email(&query.email).await {
        Ok(Some(user_to_retrieve)) => HttpResponse::Ok().json(user_to_retrieve),
        Ok(None) => HttpResponse::NotFound().finish(),
        Err(err) => {
            error!(
                operator = operator.id(),
                "Failed to retrieve user by email: {err:?}"
            );
            generic_internal_server_error()
        }
    })
}
