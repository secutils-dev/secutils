use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    notifications::EmailNotificationContent,
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
    let reset_code = users_api
        .get_data::<String>(user_id, InternalUserDataNamespace::CredentialsResetToken)
        .await?
        .with_context(|| {
            format!("User ({}) doesn't have assigned activation code. Account activation isn't possible.", *user_id)
        })?;
    let Some(user) = users_api.get(user_id).await? else {
        anyhow::bail!("User ({}) is not found.", *user_id);
    };

    // For now we send email tailored for the password reset, but eventually we can allow user
    // to reset passkey as well.
    let encoded_reset_link = format!(
        "{}reset_credentials?code={}&email={}",
        api.config.public_url.as_str(),
        urlencoding::encode(&reset_code.value),
        urlencoding::encode(&user.email)
    );

    Ok(EmailNotificationContent::html(
        "Reset password for your Secutils.dev account",
        format!("To reset your Secutils.dev password, please click the following link: {encoded_reset_link}"),
        api.templates.render("password_reset_email", &json!({ "encoded_reset_link": encoded_reset_link }))?,
    ))
}
