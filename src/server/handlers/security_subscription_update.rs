use crate::{
    security::Operator,
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::UserSubscription,
};
use actix_web::{Error, HttpResponse, Responder, post, web};
use serde::Deserialize;
use tracing::{error, info};
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(example = json!({"user_email": "user@example.com", "subscription": {"tier": "standard", "startedAt": 1700000000}}))]
pub struct UpdateSubscriptionParams {
    user_email: String,
    subscription: UserSubscription,
}

/// Updates a user's subscription (operator-only).
#[utoipa::path(
    tags = ["users"],
    request_body = UpdateSubscriptionParams,
    responses(
        (status = 204, description = "Subscription was successfully updated."),
        (status = 404, description = "User not found.")
    )
)]
#[post("/api/user/subscription")]
pub async fn security_subscription_update(
    state: web::Data<AppState>,
    body_params: web::Json<UpdateSubscriptionParams>,
    operator: Operator,
) -> impl Responder {
    let UpdateSubscriptionParams {
        user_email,
        subscription,
    } = body_params.into_inner();
    let security_api = state.api.security();
    match security_api
        .update_subscription(&user_email, subscription)
        .await
    {
        Ok(Some(updated_user)) => {
            info!(
                operator = operator.id(),
                user.id = %updated_user.id,
                "Successfully updated user subscription."
            );
            Ok::<HttpResponse, Error>(HttpResponse::NoContent().finish())
        }
        Ok(None) => {
            error!(
                operator = operator.id(),
                "Failed to find user by email (`{user_email}`)."
            );
            Ok(HttpResponse::NotFound().finish())
        }
        Err(err) => {
            error!(
                operator = operator.id(),
                "Failed to update user's tier by email (`{user_email}`): {err:?}"
            );
            Ok(generic_internal_server_error())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UpdateSubscriptionParams;
    use crate::tests::schema_example;

    #[test]
    fn update_subscription_params_example_is_valid() {
        let example: UpdateSubscriptionParams =
            serde_json::from_value(schema_example::<UpdateSubscriptionParams>()).unwrap();
        assert!(!example.user_email.is_empty());
    }
}
