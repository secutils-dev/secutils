use crate::{
    security::Operator,
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::UserId,
};
use actix_web::{Error, HttpResponse, Responder, delete, post, web};
use serde::Deserialize;
use serde_json::json;
use tracing::{error, info, warn};
use utoipa::{IntoParams, ToSchema};

#[derive(Deserialize, ToSchema)]
#[schema(example = json!({"email": "user@example.com"}))]
pub struct RemoveParams {
    /// Email of the user to remove.
    pub email: String,
}

/// Removes a user by email (operator-only).
#[utoipa::path(
    tags = ["users"],
    request_body = RemoveParams,
    responses(
        (status = 204, description = "User was successfully removed."),
        (status = BAD_REQUEST, description = "Email cannot be empty."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials."),
        (status = FORBIDDEN, description = "Caller is not an operator.")
    )
)]
#[post("/api/users/remove")]
pub async fn security_users_remove(
    state: web::Data<AppState>,
    body_params: web::Json<RemoveParams>,
    operator: Operator,
) -> impl Responder {
    let body_params = body_params.into_inner();
    if body_params.email.is_empty() {
        return Ok::<HttpResponse, Error>(
            HttpResponse::BadRequest().json(json!({ "message": "The email cannot be empty." })),
        );
    }

    let api = state.api.security();
    match api.terminate(&body_params.email).await {
        Ok(Some(user_id)) => {
            info!(
                operator = operator.id(),
                user.id = %user_id,
                "Successfully removed user.",
            );
        }
        Ok(None) => {
            warn!(operator = operator.id(), "Cannot remove non-existent user.");
        }
        Err(err) => {
            error!(operator = operator.id(), "Failed to remove user: {err:?}");
            return Ok(generic_internal_server_error());
        }
    }

    Ok(HttpResponse::NoContent().finish())
}

#[derive(Deserialize, IntoParams)]
pub struct UserIdPath {
    /// The user ID to remove.
    #[param(value_type = String, format = Uuid)]
    pub user_id: UserId,
}

/// Removes a user by ID (operator-only). Sibling of `POST /api/users/remove` for cases
/// where the operator has the user's UUID (e.g. a clone they just created) but not their
/// email.
#[utoipa::path(
    tags = ["users"],
    params(UserIdPath),
    responses(
        (status = 204, description = "User was successfully removed."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials."),
        (status = FORBIDDEN, description = "Caller is not an operator."),
        (status = NOT_FOUND, description = "User not found.")
    )
)]
#[delete("/api/users/{user_id}")]
pub async fn security_users_remove_by_id(
    state: web::Data<AppState>,
    operator: Operator,
    path: web::Path<UserIdPath>,
) -> impl Responder {
    let user_id = path.user_id;

    // Resolve the email so we can reuse `terminate(email)` (which handles Kratos + DB).
    let user_email = match state.api.users().get(user_id).await {
        Ok(Some(user)) => user.email,
        Ok(None) => {
            warn!(
                operator = operator.id(),
                user.id = %user_id,
                "Cannot remove non-existent user by ID."
            );
            return Ok::<HttpResponse, Error>(HttpResponse::NotFound().finish());
        }
        Err(err) => {
            error!(
                operator = operator.id(),
                user.id = %user_id,
                "Failed to look up user by ID for removal: {err:?}"
            );
            return Ok(generic_internal_server_error());
        }
    };

    match state.api.security().terminate(&user_email).await {
        Ok(Some(removed_id)) => {
            info!(
                operator = operator.id(),
                user.id = %removed_id,
                "Successfully removed user by ID."
            );
        }
        Ok(None) => {
            warn!(
                operator = operator.id(),
                user.id = %user_id,
                "Cannot remove non-existent user by ID (race after lookup)."
            );
        }
        Err(err) => {
            error!(
                operator = operator.id(),
                user.id = %user_id,
                "Failed to remove user by ID: {err:?}"
            );
            return Ok(generic_internal_server_error());
        }
    }

    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use super::RemoveParams;
    use crate::tests::schema_example;

    #[test]
    fn remove_params_example_is_valid() {
        let example: RemoveParams =
            serde_json::from_value(schema_example::<RemoveParams>()).unwrap();
        assert!(!example.email.is_empty());
    }
}
