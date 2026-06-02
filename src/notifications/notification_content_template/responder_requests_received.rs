use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::{
        EmailNotificationAttachment, EmailNotificationContent,
        notification_content_template::{SECUTILS_LOGO_BYTES, plain_text_footer},
    },
};
use serde_json::json;
use time::{OffsetDateTime, UtcOffset, macros::format_description};
use uuid::Uuid;

/// Human-friendly rendering of the "since" timestamp, e.g. `February 19, 2025 at 21:20 UTC`. The
/// timestamp is always normalized to UTC before formatting.
const SINCE_FORMAT: &[time::format_description::BorrowedFormatItem<'_>] =
    format_description!("[month repr:long] [day padding:none], [year] at [hour]:[minute] UTC");

/// Compiles the "responder was hit" notification as an email.
///
/// The email coalesces all requests received since `since` into a single summary. When
/// `unsubscribe_url` is provided, both the HTML and plain-text bodies append a muted footer linking
/// to it (this is a product notification, so it always carries the opt-out affordance in
/// production).
pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    responder_id: Uuid,
    responder_name: &str,
    request_count: usize,
    since: OffsetDateTime,
    unsubscribe_url: Option<&str>,
) -> anyhow::Result<EmailNotificationContent> {
    let back_link = format!(
        "{}ws/webhooks__responders?q={}",
        api.config.public_url, responder_id
    );
    let since = since
        .to_offset(UtcOffset::UTC)
        .format(SINCE_FORMAT)
        .unwrap_or_default();
    let requests_label = if request_count == 1 {
        "1 request".to_string()
    } else {
        format!("{request_count} requests")
    };

    let subject = format!("[Secutils.dev] Responder was hit: \"{responder_name}\"");
    let mut text = format!(
        "Your \"{responder_name}\" responder received {requests_label} since {since}. Visit {back_link} to learn more."
    );
    let html = api.templates.render(
        "responder_requests_received_email",
        &json!({
            "responder_name": responder_name,
            "requests_label": requests_label,
            "since": since,
            "back_link": back_link,
            "home_link": api.config.public_url.as_str(),
            "unsubscribe_url": unsubscribe_url,
        }),
    )?;

    if let Some(url) = unsubscribe_url {
        text.push_str(&plain_text_footer(url));
    }

    Ok(EmailNotificationContent::html_with_attachments(
        subject,
        text,
        html,
        vec![EmailNotificationAttachment::inline(
            "secutils-logo",
            "image/png",
            SECUTILS_LOGO_BYTES.to_vec(),
        )],
    ))
}
