use crate::{
    security::Operator,
    server::{StatusLevel, app_state::AppState},
};
use actix_web::{HttpResponse, Responder, error::ErrorInternalServerError, post, web};
use anyhow::anyhow;
use serde::Deserialize;
use tracing::error;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
#[schema(example = json!({"level": "available"}))]
pub struct SetStatusAPIParams {
    pub level: StatusLevel,
}

/// Sets the server status level (operator-only).
#[utoipa::path(
    tags = ["status"],
    request_body = SetStatusAPIParams,
    responses(
        (status = 204, description = "Status was successfully updated."),
        (status = 500, description = "Failed to update status.")
    )
)]
#[post("/api/status")]
pub async fn status_set(
    state: web::Data<AppState>,
    body_params: web::Json<SetStatusAPIParams>,
    operator: Operator,
) -> impl Responder {
    state
        .status
        .write()
        .map(|mut status| {
            status.level = body_params.level;
            HttpResponse::NoContent().finish()
        })
        .map_err(|err| {
            error!(
                operator = operator.id(),
                "Failed to set server status: {err:?}."
            );
            ErrorInternalServerError(anyhow!("Failed to set server status: {:?}.", err))
        })
}

#[cfg(test)]
mod tests {
    use super::SetStatusAPIParams;
    use crate::{server::StatusLevel, tests::schema_example};

    #[test]
    fn set_status_params_example_is_valid() {
        let example: SetStatusAPIParams =
            serde_json::from_value(schema_example::<SetStatusAPIParams>()).unwrap();
        assert!(
            example.level == StatusLevel::Available || example.level == StatusLevel::Unavailable
        );
    }
}
