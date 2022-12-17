use crate::api::EmailBody;
use std::time::SystemTime;

pub struct Email {
    pub to: String,
    pub subject: String,
    pub body: EmailBody,
    pub timestamp: Option<SystemTime>,
}

impl Email {
    pub fn new<R: Into<String>, S: Into<String>>(to: R, subject: S, body: EmailBody) -> Self {
        Self {
            to: to.into(),
            subject: subject.into(),
            body,
            timestamp: None,
        }
    }

    /// Create `Email` instance with the specified timestamp.
    pub fn with_timestamp(self, timestamp: SystemTime) -> Self {
        Self {
            timestamp: Some(timestamp),
            ..self
        }
    }
}
