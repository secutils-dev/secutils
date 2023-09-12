use serde::{Deserialize, Serialize};

/// Describes the disposition of a email notification content attachment with an arbitrary ID.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum EmailNotificationAttachmentDisposition {
    /// Notification email attachment should be inlined.
    Inline(String),
}

#[cfg(test)]
mod tests {
    use super::EmailNotificationAttachmentDisposition;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::to_stdvec(&EmailNotificationAttachmentDisposition::Inline(
                "abc".to_string()
            ))?,
            vec![0, 3, 97, 98, 99]
        );

        Ok(())
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            postcard::from_bytes::<EmailNotificationAttachmentDisposition>(&[0, 3, 97, 98, 99])?,
            EmailNotificationAttachmentDisposition::Inline("abc".to_string())
        );

        Ok(())
    }
}
