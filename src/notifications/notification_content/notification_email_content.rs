use crate::notifications::NotificationContent;
use serde::{Deserialize, Serialize};

/// Describes the content of the email notification.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct NotificationEmailContent {
    /// Email subject.
    pub subject: String,
    /// Email body in plain text (used as a fallback if `html` is specified).
    pub text: String,
    /// Email body in HTML.
    pub html: Option<String>,
}

impl NotificationEmailContent {
    /// Creates a new plain-text email.
    pub fn text<S: Into<String>, T: Into<String>>(subject: S, text: T) -> Self {
        Self {
            subject: subject.into(),
            text: text.into(),
            html: None,
        }
    }

    /// Create new HTML email with a plain-text fallback.
    pub fn html<S: Into<String>, T: Into<String>, H: Into<String>>(
        subject: S,
        text: T,
        html: H,
    ) -> Self {
        Self {
            subject: subject.into(),
            text: text.into(),
            html: Some(html.into()),
        }
    }
}

/// Defines how any `NotificationContent` can be converted to `NotificationEmailContent`.
impl From<NotificationContent> for NotificationEmailContent {
    fn from(content: NotificationContent) -> Self {
        match content {
            NotificationContent::Text(text) => Self::text("[NO SUBJECT]", text),
            NotificationContent::Email(email) => email,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NotificationEmailContent;
    use crate::notifications::NotificationContent;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::to_stdvec(&NotificationEmailContent::text("subject", "text"))?,
            vec![7, 115, 117, 98, 106, 101, 99, 116, 4, 116, 101, 120, 116, 0]
        );

        assert_eq!(
            postcard::to_stdvec(&NotificationEmailContent::html("subject", "text", "html"))?,
            vec![
                7, 115, 117, 98, 106, 101, 99, 116, 4, 116, 101, 120, 116, 1, 4, 104, 116, 109, 108
            ]
        );

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::from_bytes::<NotificationEmailContent>(&[
                7, 115, 117, 98, 106, 101, 99, 116, 4, 116, 101, 120, 116, 0
            ])?,
            NotificationEmailContent::text("subject", "text")
        );

        assert_eq!(
            postcard::from_bytes::<NotificationEmailContent>(&[
                7, 115, 117, 98, 106, 101, 99, 116, 4, 116, 101, 120, 116, 1, 4, 104, 116, 109, 108
            ])?,
            NotificationEmailContent::html("subject", "text", "html")
        );

        Ok(())
    }

    #[test]
    fn new_text_email() {
        assert_eq!(
            NotificationEmailContent::text("subject", "text"),
            NotificationEmailContent {
                subject: "subject".to_string(),
                text: "text".to_string(),
                html: None,
            }
        );
    }

    #[test]
    fn new_html_email() {
        assert_eq!(
            NotificationEmailContent::html("subject", "text", "html"),
            NotificationEmailContent {
                subject: "subject".to_string(),
                text: "text".to_string(),
                html: Some("html".to_string()),
            }
        );
    }

    #[test]
    fn convert_to_email_content() {
        assert_eq!(
            NotificationEmailContent::from(NotificationContent::Text("text".to_string())),
            NotificationEmailContent {
                subject: "[NO SUBJECT]".to_string(),
                text: "text".to_string(),
                html: None,
            }
        );

        assert_eq!(
            NotificationEmailContent::from(NotificationContent::Email(
                NotificationEmailContent::text("subject", "text")
            )),
            NotificationEmailContent {
                subject: "subject".to_string(),
                text: "text".to_string(),
                html: None,
            }
        );

        assert_eq!(
            NotificationEmailContent::from(NotificationContent::Email(
                NotificationEmailContent::html("subject", "text", "html")
            )),
            NotificationEmailContent {
                subject: "subject".to_string(),
                text: "text".to_string(),
                html: Some("html".to_string()),
            }
        );
    }
}
