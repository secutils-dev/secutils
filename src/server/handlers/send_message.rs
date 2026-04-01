use crate::{
    error::Error as SecutilsError,
    notifications::{EmailNotificationContent, NotificationContent, NotificationDestination},
    security::Operator,
    server::app_state::AppState,
    users::User,
};
use actix_web::{HttpResponse, post, web};
use serde::Deserialize;
use time::OffsetDateTime;
use tracing::{error, info};
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
#[schema(example = json!({"message": "I'd like to request a feature.", "email": "user@example.com"}))]
pub struct SendMessageParams {
    /// The message text.
    pub message: String,
    /// Optional sender email address.
    pub email: Option<String>,
}

/// Sends a contact message via email.
#[utoipa::path(
    tags = ["messages"],
    request_body = SendMessageParams,
    security((), ("bearerAuth" = [])),
    responses(
        (status = 204, description = "Message was successfully sent."),
        (status = 403, description = "SMTP is not configured.")
    )
)]
#[post("/api/send_message")]
pub async fn send_message(
    state: web::Data<AppState>,
    body_params: web::Json<SendMessageParams>,
    operator: Option<Operator>,
    user: Option<User>,
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
            error!("SMTP isn't configured.");
            return Err(SecutilsError::access_forbidden());
        }
    };

    info!(
        operator = operator.as_ref().map(|operator| operator.id()),
        user.id = user.as_ref().map(|user| user.id.to_string()),
        "Sending a message `{body}`."
    );

    let notifications = state.api.notifications();
    notifications
        .schedule_notification(
            NotificationDestination::Email(recipient),
            NotificationContent::Email(EmailNotificationContent::text(
                "Secutils contact request",
                &body,
            )),
            OffsetDateTime::now_utc(),
        )
        .await?;

    info!(
        operator = operator.as_ref().map(|operator| operator.id()),
        user.id = user.as_ref().map(|user| user.id.to_string()),
        "Successfully sent message."
    );
    Ok(HttpResponse::NoContent().finish())
}

#[cfg(test)]
mod tests {
    use super::SendMessageParams;
    use crate::tests::schema_example;

    #[test]
    fn send_message_params_example_is_valid() {
        let example: SendMessageParams =
            serde_json::from_value(schema_example::<SendMessageParams>()).unwrap();
        assert!(!example.message.is_empty());
    }
}
