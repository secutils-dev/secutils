use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::{
        notification_content_template::SECUTILS_LOGO_BYTES, EmailNotificationAttachment,
        EmailNotificationContent,
    },
    users::{InternalUserDataNamespace, UserId},
};
use anyhow::Context;
use serde_json::json;

/// Compiles account activation template as an email.
pub async fn compile_to_email<DR: DnsResolver, ET: EmailTransport>(
    api: &Api<DR, ET>,
    user_id: UserId,
) -> anyhow::Result<EmailNotificationContent> {
    let users_api = api.users();
    let activation_code = users_api
        .get_data::<String>(user_id, InternalUserDataNamespace::AccountActivationToken)
        .await?
        .with_context(|| {
            format!("User ({}) doesn't have assigned activation code. Account activation isn't possible.", *user_id)
        })?;
    let Some(user) = users_api.get(user_id).await? else {
        anyhow::bail!("User ({}) is not found.", *user_id);
    };

    let encoded_activation_link = format!(
        "{}activate?code={}&email={}",
        api.config.public_url.as_str(),
        urlencoding::encode(&activation_code.value),
        urlencoding::encode(&user.email)
    );

    Ok(EmailNotificationContent::html_with_attachments(
        "Activate your Secutils.dev account",
        format!("To activate your Secutils.dev account, please use the following link: {encoded_activation_link}"),
        api.templates.render(
            "account_activation_email", 
            &json!({ "encoded_activation_link": encoded_activation_link, "home_link": api.config.public_url.as_str() })
        )?,
        vec![EmailNotificationAttachment::inline(
            "secutils-logo",
            "image/png",
            SECUTILS_LOGO_BYTES.to_vec(),
        )]
    ))
}
