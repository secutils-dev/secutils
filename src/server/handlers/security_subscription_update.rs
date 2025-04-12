use crate::{
    security::Operator,
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::UserSubscription,
};
use actix_web::{Error, HttpResponse, Responder, web};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct UpdateSubscriptionParams {
    user_email: String,
    subscription: UserSubscription,
}

/// Updates user's subscription.
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
            log::info!(
                operator:serde = operator.id(),
                user:serde = updated_user.log_context();
                "Successfully updated user subscription."
            );
            Ok::<HttpResponse, Error>(HttpResponse::NoContent().finish())
        }
        Ok(None) => {
            log::error!(operator:serde = operator.id(); "Failed to find user by email (`{user_email}`).");
            Ok(HttpResponse::NotFound().finish())
        }
        Err(err) => {
            log::error!(operator:serde = operator.id(); "Failed to update user's tier by email (`{user_email}`): {err:?}");
            Ok(generic_internal_server_error())
        }
    }
}
