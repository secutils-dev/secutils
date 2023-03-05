use crate::{
    api::{Email, EmailBody},
    error::SecutilsError,
    server::app_state::AppState,
};
use actix_web::{web, HttpResponse};
use serde::Deserialize;
use serde_json::json;

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

    let email = match state
        .config
        .smtp
        .as_ref()
        .and_then(|smtp| smtp.catch_all_recipient.as_ref())
    {
        Some(recipient) => Email::new(
            recipient,
            "Secutils contact request",
            EmailBody::Text(body.clone()),
        ),
        None => {
            log::error!("SMTP isn't configured.");
            return Ok(HttpResponse::InternalServerError().json(json!({ "status": "failed" })));
        }
    };

    state.api.emails().send(email)?;

    log::info!("Successfully sent message `{}`", body);
    Ok(HttpResponse::NoContent().finish())
}
