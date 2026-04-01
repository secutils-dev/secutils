use crate::{
    security::Operator,
    server::{app_state::AppState, http_errors::generic_internal_server_error},
};
use actix_web::{Error, HttpResponse, Responder, post, web};
use serde::Deserialize;
use serde_json::json;
use tracing::{error, info, warn};
use utoipa::ToSchema;

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
