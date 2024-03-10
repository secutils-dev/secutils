use crate::{
    server::{app_state::AppState, http_errors::generic_internal_server_error},
    users::{User, UserSubscription},
};
use actix_web::{web, Error, HttpResponse, Responder};
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
    user: User,
) -> impl Responder {
    state.ensure_admin(&user)?;

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
            log::info!(user:serde = updated_user.log_context(); "Successfully updated user subscription.");
            Ok::<HttpResponse, Error>(HttpResponse::NoContent().finish())
        }
        Ok(None) => {
            log::error!("Failed to find user by email (`{user_email}`).");
            Ok(HttpResponse::NotFound().finish())
        }
        Err(err) => {
            log::error!("Failed to update user's tier by email (`{user_email}`): {err:?}");
            Ok(generic_internal_server_error())
        }
    }
}
