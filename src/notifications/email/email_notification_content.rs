use crate::notifications::EmailNotificationAttachment;
use serde::{Deserialize, Serialize};

/// Describes the content of the email notification.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EmailNotificationContent {
    /// Email subject.
    pub subject: String,
    /// Email body in plain text (used as a fallback if `html` is specified).
    pub text: String,
    /// Email body in HTML.
    pub html: Option<String>,
    /// Optional list of email attachments.
    pub attachments: Option<Vec<EmailNotificationAttachment>>,
}

impl EmailNotificationContent {
    /// Creates a new plain-text email.
    pub fn text<S: Into<String>, T: Into<String>>(subject: S, text: T) -> Self {
        Self {
            subject: subject.into(),
            text: text.into(),
            html: None,
            attachments: None,
        }
    }

    /// Create a new HTML email with a plain-text fallback.
    #[allow(dead_code)]
    pub fn html<S: Into<String>, T: Into<String>, H: Into<String>>(
        subject: S,
        text: T,
        html: H,
    ) -> Self {
        Self {
            subject: subject.into(),
            text: text.into(),
            html: Some(html.into()),
            attachments: None,
        }
    }

    /// Create a new HTML email with a plain-text fallback and attachments.
    pub fn html_with_attachments<S: Into<String>, T: Into<String>, H: Into<String>>(
        subject: S,
        text: T,
        html: H,
        attachments: Vec<EmailNotificationAttachment>,
    ) -> Self {
        Self {
            subject: subject.into(),
            text: text.into(),
            html: Some(html.into()),
            attachments: Some(attachments),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EmailNotificationContent;
    use crate::notifications::EmailNotificationAttachment;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::to_stdvec(&EmailNotificationContent::text("subject", "text"))?,
            vec![
                7, 115, 117, 98, 106, 101, 99, 116, 4, 116, 101, 120, 116, 0, 0
            ]
        );

        assert_eq!(
            postcard::to_stdvec(&EmailNotificationContent::html("subject", "text", "html"))?,
            vec![
                7, 115, 117, 98, 106, 101, 99, 116, 4, 116, 101, 120, 116, 1, 4, 104, 116, 109,
                108, 0
            ]
        );

        assert_eq!(
            postcard::to_stdvec(&EmailNotificationContent::html_with_attachments(
                "subject",
                "text",
                "html",
                vec![EmailNotificationAttachment::inline(
                    "cid",
                    "text/plain",
                    vec![1, 2, 3]
                )]
            ))?,
            vec![
                7, 115, 117, 98, 106, 101, 99, 116, 4, 116, 101, 120, 116, 1, 4, 104, 116, 109,
                108, 1, 1, 0, 3, 99, 105, 100, 10, 116, 101, 120, 116, 47, 112, 108, 97, 105, 110,
                3, 1, 2, 3
            ]
        );

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::from_bytes::<EmailNotificationContent>(&[
                7, 115, 117, 98, 106, 101, 99, 116, 4, 116, 101, 120, 116, 0, 0
            ])?,
            EmailNotificationContent::text("subject", "text")
        );

        assert_eq!(
            postcard::from_bytes::<EmailNotificationContent>(&[
                7, 115, 117, 98, 106, 101, 99, 116, 4, 116, 101, 120, 116, 1, 4, 104, 116, 109,
                108, 0
            ])?,
            EmailNotificationContent::html("subject", "text", "html")
        );

        assert_eq!(
            postcard::from_bytes::<EmailNotificationContent>(&[
                7, 115, 117, 98, 106, 101, 99, 116, 4, 116, 101, 120, 116, 1, 4, 104, 116, 109,
                108, 1, 1, 0, 3, 99, 105, 100, 10, 116, 101, 120, 116, 47, 112, 108, 97, 105, 110,
                3, 1, 2, 3
            ])?,
            EmailNotificationContent::html_with_attachments(
                "subject",
                "text",
                "html",
                vec![EmailNotificationAttachment::inline(
                    "cid",
                    "text/plain",
                    vec![1, 2, 3]
                )]
            )
        );

        Ok(())
    }

    #[test]
    fn new_text_email() {
        assert_eq!(
            EmailNotificationContent::text("subject", "text"),
            EmailNotificationContent {
                subject: "subject".to_string(),
                text: "text".to_string(),
                html: None,
                attachments: None,
            }
        );
    }

    #[test]
    fn new_html_email() {
        assert_eq!(
            EmailNotificationContent::html("subject", "text", "html"),
            EmailNotificationContent {
                subject: "subject".to_string(),
                text: "text".to_string(),
                html: Some("html".to_string()),
                attachments: None,
            }
        );
    }

    #[test]
    fn new_html_email_with_attachments() {
        assert_eq!(
            EmailNotificationContent::html_with_attachments(
                "subject",
                "text",
                "html",
                vec![EmailNotificationAttachment::inline(
                    "cid",
                    "text/plain",
                    vec![1, 2, 3]
                )]
            ),
            EmailNotificationContent {
                subject: "subject".to_string(),
                text: "text".to_string(),
                html: Some("html".to_string()),
                attachments: Some(vec![EmailNotificationAttachment::inline(
                    "cid",
                    "text/plain",
                    vec![1, 2, 3]
                )]),
            }
        );
    }
}
