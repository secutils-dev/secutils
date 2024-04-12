use crate::{
    error::Error as SecutilsError,
    notifications::{NotificationContent, NotificationContentTemplate, NotificationDestination},
    server::AppState,
};
use actix_web::{web, HttpResponse};
use serde_derive::Deserialize;
use std::{collections::HashMap, str::FromStr};

use crate::{
    logging::UserLogContext,
    security::kratos::{EmailTemplateType, Identity},
};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

/// Kratos email request, see https://github.com/ory/kratos/blob/master/courier/stub/request.config.mailer.jsonnet
#[derive(Deserialize, Debug)]
pub struct EmailParams {
    recipient: String,
    template_type: String,
    identity: Option<Identity>,
    verification_url: Option<Url>,
    recovery_code: Option<String>,
}

pub async fn security_users_email(
    state: web::Data<AppState>,
    body_params: web::Json<EmailParams>,
) -> Result<HttpResponse, SecutilsError> {
    let kratos_email = body_params.into_inner();

    let identity_context = kratos_email
        .identity
        .as_ref()
        .map(|identity| UserLogContext::new(identity.id.into()));
    let (destination, content) = match parse_email_params(&kratos_email) {
        Ok(content) => {
            log::info!(
                user:serde = identity_context;
                "Received Kratos {} email request for {}.",
                kratos_email.template_type,
                kratos_email.recipient,
            );
            (
                NotificationDestination::Email(kratos_email.recipient.clone()),
                content,
            )
        }
        Err(err) => {
            log::error!(
                user:serde = identity_context;
                "Received unsupported ({}) Kratos email request for {}: {err:?}.",
                kratos_email.template_type,
                kratos_email.recipient,
            );
            (
                match state
                    .config
                    .smtp
                    .as_ref()
                    .and_then(|smtp| Some(smtp.catch_all.as_ref()?.recipient.clone()))
                {
                    Some(recipient) => NotificationDestination::Email(recipient),
                    None => NotificationDestination::ServerLog,
                },
                NotificationContent::Text(format!(
                    "Unsupported Kratos email request: {kratos_email:?}."
                )),
            )
        }
    };

    if let Err(err) = state
        .api
        .notifications()
        .schedule_notification(destination, content, OffsetDateTime::now_utc())
        .await
    {
        log::error!("Failed to schedule Kratos email notification: {err:?}");
    }

    Ok(HttpResponse::NoContent().finish())
}

fn parse_email_params(email: &EmailParams) -> anyhow::Result<NotificationContent> {
    match EmailTemplateType::from_str(&email.template_type)? {
        EmailTemplateType::RecoveryCode => {
            let recovery_code = email
                .recovery_code
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing recovery code."))?;
            Ok(NotificationContent::Template(
                NotificationContentTemplate::AccountRecovery {
                    code: recovery_code.clone(),
                },
            ))
        }
        EmailTemplateType::VerificationCode => {
            let verification_url = email
                .verification_url
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing verification URL."))?;
            let mut query_map = verification_url
                .query_pairs()
                .into_iter()
                .collect::<HashMap<_, _>>();

            Ok(NotificationContent::Template(
                NotificationContentTemplate::AccountActivation {
                    flow_id: Uuid::from_str(
                        &query_map
                            .remove("flow")
                            .ok_or_else(|| anyhow::anyhow!("Missing verification flow."))?,
                    )?,
                    code: query_map
                        .remove("code")
                        .ok_or_else(|| anyhow::anyhow!("Missing verification code."))?
                        .into_owned(),
                },
            ))
        }
    }
}
