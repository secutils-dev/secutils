use crate::{
    error::Error,
    server::app_state::AppState,
    users::{
        NotificationEmailSetParams, NotificationEmailVerifyParams, User,
        UserNotificationDestination,
    },
};
use actix_web::{HttpResponse, delete, get, post, put, web};

/// Returns the user's notification email destination, or `null` if none is configured.
#[utoipa::path(
    tags = ["settings"],
    responses(
        (status = 200, description = "Current notification email destination, or null when unset.", body = Option<UserNotificationDestination>),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[get("/api/user/notification_email")]
pub async fn user_notification_email_get(
    state: web::Data<AppState>,
    user: User,
) -> Result<HttpResponse, Error> {
    let record = state
        .api
        .notification_destinations(&user)
        .get_email()
        .await?;
    Ok(HttpResponse::Ok().json(record))
}

/// Begins (or restarts) verification for the supplied notification email. The address is
/// persisted in a "pending" state and a 6-digit verification code is emailed to it. Until the
/// code is entered via `_verify`, notifications continue to be delivered to the login email.
#[utoipa::path(
    tags = ["settings"],
    request_body = NotificationEmailSetParams,
    responses(
        (status = 200, description = "Verification email scheduled.", body = UserNotificationDestination),
        (status = BAD_REQUEST, description = "Invalid email or attempted to set the override to the login email."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[put("/api/user/notification_email")]
pub async fn user_notification_email_set(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<NotificationEmailSetParams>,
) -> Result<HttpResponse, Error> {
    let record = state
        .api
        .notification_destinations(&user)
        .set_email(body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(record))
}

/// Submits the 6-digit code emailed by `set`/`_resend`. On success the destination is marked
/// verified and starts receiving notifications.
#[utoipa::path(
    tags = ["settings"],
    request_body = NotificationEmailVerifyParams,
    responses(
        (status = 200, description = "Notification email verified.", body = UserNotificationDestination),
        (status = BAD_REQUEST, description = "Invalid or expired code, or no verification is in progress."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials."),
        (status = NOT_FOUND, description = "No notification email is configured.")
    )
)]
#[post("/api/user/notification_email/_verify")]
pub async fn user_notification_email_verify(
    state: web::Data<AppState>,
    user: User,
    body: web::Json<NotificationEmailVerifyParams>,
) -> Result<HttpResponse, Error> {
    let record = state
        .api
        .notification_destinations(&user)
        .verify_email(body.into_inner())
        .await?;
    Ok(HttpResponse::Ok().json(record))
}

/// Re-sends the active verification code. Subject to a 1-minute cooldown and a 5/hour cap.
#[utoipa::path(
    tags = ["settings"],
    responses(
        (status = 204, description = "Verification email re-scheduled."),
        (status = BAD_REQUEST, description = "Already verified, code expired, or rate limit exceeded."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials."),
        (status = NOT_FOUND, description = "No notification email is configured.")
    )
)]
#[post("/api/user/notification_email/_resend")]
pub async fn user_notification_email_resend(
    state: web::Data<AppState>,
    user: User,
) -> Result<HttpResponse, Error> {
    state
        .api
        .notification_destinations(&user)
        .resend_verification()
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Removes the user's notification email. Routing falls back to the login email immediately.
#[utoipa::path(
    tags = ["settings"],
    responses(
        (status = 204, description = "Notification email removed."),
        (status = UNAUTHORIZED, description = "Missing or invalid authentication credentials.")
    )
)]
#[delete("/api/user/notification_email")]
pub async fn user_notification_email_delete(
    state: web::Data<AppState>,
    user: User,
) -> Result<HttpResponse, Error> {
    state
        .api
        .notification_destinations(&user)
        .clear_email()
        .await?;
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::schema_example;

    #[test]
    fn notification_email_set_params_example_is_valid() {
        let example: NotificationEmailSetParams =
            serde_json::from_value(schema_example::<NotificationEmailSetParams>()).unwrap();
        assert!(example.email.contains('@'));
    }

    #[test]
    fn notification_email_verify_params_example_is_valid() {
        let example: NotificationEmailVerifyParams =
            serde_json::from_value(schema_example::<NotificationEmailVerifyParams>()).unwrap();
        assert_eq!(example.code.len(), 6);
        assert!(example.code.chars().all(|c| c.is_ascii_digit()));
    }
}
