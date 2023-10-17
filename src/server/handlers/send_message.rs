use crate::{
    error::Error as SecutilsError,
    notifications::{EmailNotificationContent, NotificationContent, NotificationDestination},
    server::app_state::AppState,
};
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use time::OffsetDateTime;

#[derive(Deserialize)]
pub struct SendMessageParams {
    pub message: String,
    pub email: Option<String>,
}

pub async fn send_message(
    state: web::Data<AppState>,
    body_params: web::Json<SendMessageParams>,
) -> Result<HttpResponse, SecutilsError> {
    let body = if let Some(ref email) = body_params.email {
        format!("{}:{}", body_params.message, email)
    } else {
        body_params.message.to_string()
    };

    let recipient = match state
        .config
        .smtp
        .as_ref()
        .and_then(|smtp| Some(smtp.catch_all.as_ref()?.recipient.clone()))
    {
        Some(recipient) => recipient,
        None => {
            log::error!("SMTP isn't configured.");
            return Err(SecutilsError::access_forbidden());
        }
    };

    state
        .api
        .notifications()
        .schedule_notification(
            NotificationDestination::Email(recipient),
            NotificationContent::Email(EmailNotificationContent::text(
                "Secutils contact request",
                &body,
            )),
            OffsetDateTime::now_utc(),
        )
        .await?;

    log::info!("Successfully sent message `{}`", body);
    Ok(HttpResponse::NoContent().finish())
}
