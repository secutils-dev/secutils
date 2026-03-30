use crate::{
    security::{Operator, kratos::Identity},
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::{SubscriptionTier, User, UserId, UserSignupError, UserSubscription},
};
use actix_web::{HttpResponse, Responder, post, web};
use serde::Deserialize;
use serde_json::json;
use std::ops::Add;
use tracing::{error, info};
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
#[schema(example = json!({"identity": {"id": "00000000-0000-0000-0000-000000000001", "traits": {"email": "user@example.com"}, "verifiable_addresses": [{"value": "user@example.com", "verified": false}], "created_at": "2025-01-01T00:00:00Z"}}))]
pub struct SignupParams {
    /// The Kratos identity for the new user.
    pub identity: Identity,
}

/// Signs up a new user with the provided identity (operator-only).
#[utoipa::path(
    tags = ["users"],
    request_body = SignupParams,
    responses(
        (status = 200, description = "User was successfully signed up."),
        (status = BAD_REQUEST, description = "Email already registered.")
    )
)]
#[post("/api/users/signup")]
pub async fn security_users_signup(
    state: web::Data<AppState>,
    operator: Operator,
    body_params: web::Json<SignupParams>,
) -> impl Responder {
    let body_params = body_params.into_inner();
    let email = body_params.identity.traits.email.to_lowercase();
    let trial_end = body_params
        .identity
        .created_at
        .add(UserSubscription::TRIAL_LENGTH);

    let security_api = state.api.security();
    let preconfigured_users = state.config.security.preconfigured_users.as_ref();
    let (handle, subscription) = match preconfigured_users.and_then(|users| users.get(&email)) {
        Some(preconfigured_user) => (
            preconfigured_user.handle.clone(),
            UserSubscription {
                tier: preconfigured_user.tier,
                started_at: body_params.identity.created_at,
                ends_at: None,
                trial_started_at: Some(body_params.identity.created_at),
                trial_ends_at: Some(trial_end),
            },
        ),
        None => (
            match security_api.generate_user_handle().await {
                Ok(handle) => handle,
                Err(err) => {
                    error!(
                        operator = operator.id(),
                        "Failed to generate user handle: {err:?}"
                    );
                    return generic_internal_server_error();
                }
            },
            // Signup user with a basic subscription by default and activate trial.
            UserSubscription {
                tier: SubscriptionTier::Basic,
                started_at: body_params.identity.created_at,
                ends_at: None,
                trial_started_at: Some(body_params.identity.created_at),
                trial_ends_at: Some(trial_end),
            },
        ),
    };

    let user = User {
        id: UserId::from(body_params.identity.id),
        email,
        handle,
        created_at: body_params.identity.created_at,
        is_activated: body_params.identity.is_activated(),
        is_operator: false,
        subscription,
    };

    match security_api.signup(&user).await {
        Ok(_) => {
            info!(
                operator = operator.id(),
                user.id = %user.id,
                "Successfully signed up a new user."
            );
            HttpResponse::Ok().finish()
        }
        Err(err) => {
            error!(
                operator = operator.id(),
                user.id = %user.id,
                "Failed to signup a user: {err:?}"
            );
            match err.downcast_ref::<UserSignupError>() {
                Some(err) => match err {
                    UserSignupError::EmailAlreadyRegistered => HttpResponse::BadRequest().json(
                        json!({ "message": "The email address is already registered. Please try signing in or use a different email address." })
                    )
                },
                None => generic_internal_server_error(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SignupParams;
    use crate::tests::schema_example;

    #[test]
    fn signup_params_example_is_valid() {
        let example: SignupParams =
            serde_json::from_value(schema_example::<SignupParams>()).unwrap();
        assert!(!example.identity.traits.email.is_empty());
    }
}
