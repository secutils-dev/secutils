use crate::{
    api::{Email, EmailBody},
    Config, SmtpConfig,
};
use anyhow::{bail, Context};
use lettre::{
    message::{header::ContentType, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    Message, SmtpTransport, Transport,
};

#[derive(Clone, Debug)]
pub struct EmailsApi<C: AsRef<Config>> {
    config: C,
}

impl<C: AsRef<Config>> EmailsApi<C> {
    pub fn new(config: C) -> Self {
        Self { config }
    }

    pub fn send(&self, email: Email) -> anyhow::Result<()> {
        let smtp_config = if let Some(ref smtp_config) = self.config.as_ref().smtp {
            smtp_config
        } else {
            bail!("SMTP is not configured.");
        };

        Self::build_transport(smtp_config)?
            .send(&Self::build_message(email, smtp_config)?)
            .with_context(|| "Failed to send email".to_string())
            .map(|_| ())
    }

    fn build_message(email: Email, smtp_config: &SmtpConfig) -> anyhow::Result<Message> {
        let recipient = if let Some(ref catch_all) = smtp_config.catch_all_recipient {
            catch_all.parse()?
        } else {
            email
                .to
                .parse()
                .with_context(|| format!("Cannot parse TO address: {}", email.to))?
        };

        let message_builder = Message::builder()
            .from(smtp_config.username.parse()?)
            .reply_to(smtp_config.username.parse()?)
            .to(recipient)
            .subject(email.subject);

        let message_builder = if let Some(date) = email.timestamp {
            message_builder.date(date)
        } else {
            message_builder
        };

        match email.body {
            EmailBody::Text(content) => Ok(message_builder.body(content)?),
            EmailBody::Html { content, fallback } => Ok(message_builder.multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(fallback),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(content),
                    ),
            )?),
        }
    }

    fn build_transport(smtp_config: &SmtpConfig) -> anyhow::Result<SmtpTransport> {
        Ok(SmtpTransport::relay(&smtp_config.address)?
            .credentials(Credentials::new(
                smtp_config.username.clone(),
                smtp_config.password.clone(),
            ))
            .build())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        api::{Email, EmailBody, EmailsApi},
        config::SmtpConfig,
        Config,
    };
    use insta::assert_debug_snapshot;
    use std::time::SystemTime;
    use time::OffsetDateTime;

    #[test]
    fn can_build_text_message() -> anyhow::Result<()> {
        let config = SmtpConfig {
            username: "smtp@secutils.dev".to_string(),
            password: "changeme".to_string(),
            address: "smtp_server.secutils.dev".to_string(),
            catch_all_recipient: None,
        };

        let message = EmailsApi::<&Config>::build_message(
            Email::new(
                "dev@secutils.dev",
                "subject",
                EmailBody::Text("Text body".to_string()),
            )
            .with_timestamp(SystemTime::from(OffsetDateTime::from_unix_timestamp(
                946720800,
            )?)),
            &config,
        )?;

        assert_debug_snapshot!(message, @r###"
        Message {
            headers: Headers {
                headers: [
                    HeaderValue {
                        name: HeaderName(
                            "From",
                        ),
                        raw_value: "smtp@secutils.dev",
                        encoded_value: "smtp@secutils.dev",
                    },
                    HeaderValue {
                        name: HeaderName(
                            "Reply-To",
                        ),
                        raw_value: "smtp@secutils.dev",
                        encoded_value: "smtp@secutils.dev",
                    },
                    HeaderValue {
                        name: HeaderName(
                            "To",
                        ),
                        raw_value: "dev@secutils.dev",
                        encoded_value: "dev@secutils.dev",
                    },
                    HeaderValue {
                        name: HeaderName(
                            "Subject",
                        ),
                        raw_value: "subject",
                        encoded_value: "subject",
                    },
                    HeaderValue {
                        name: HeaderName(
                            "Date",
                        ),
                        raw_value: "Sat, 01 Jan 2000 10:00:00 +0000",
                        encoded_value: "Sat, 01 Jan 2000 10:00:00 +0000",
                    },
                    HeaderValue {
                        name: HeaderName(
                            "Content-Transfer-Encoding",
                        ),
                        raw_value: "7bit",
                        encoded_value: "7bit",
                    },
                ],
            },
            body: Raw(
                [
                    84,
                    101,
                    120,
                    116,
                    32,
                    98,
                    111,
                    100,
                    121,
                ],
            ),
            envelope: Envelope {
                forward_path: [
                    Address {
                        serialized: "dev@secutils.dev",
                        at_start: 3,
                    },
                ],
                reverse_path: Some(
                    Address {
                        serialized: "smtp@secutils.dev",
                        at_start: 4,
                    },
                ),
            },
        }
        "###);

        Ok(())
    }

    #[test]
    fn can_build_html_message() -> anyhow::Result<()> {
        let config = SmtpConfig {
            username: "smtp@secutils.dev".to_string(),
            password: "changeme".to_string(),
            address: "smtp_server.secutils.dev".to_string(),
            catch_all_recipient: None,
        };

        let message = EmailsApi::<&Config>::build_message(
            Email::new(
                "dev@secutils.dev",
                "subject",
                EmailBody::Html {
                    content: "<b>Text body</b>".to_string(),
                    fallback: "Text body".to_string(),
                },
            )
            .with_timestamp(SystemTime::from(OffsetDateTime::from_unix_timestamp(
                946720800,
            )?)),
            &config,
        )?;

        assert_debug_snapshot!(message.headers(), @r###"
        Headers {
            headers: [
                HeaderValue {
                    name: HeaderName(
                        "From",
                    ),
                    raw_value: "smtp@secutils.dev",
                    encoded_value: "smtp@secutils.dev",
                },
                HeaderValue {
                    name: HeaderName(
                        "Reply-To",
                    ),
                    raw_value: "smtp@secutils.dev",
                    encoded_value: "smtp@secutils.dev",
                },
                HeaderValue {
                    name: HeaderName(
                        "To",
                    ),
                    raw_value: "dev@secutils.dev",
                    encoded_value: "dev@secutils.dev",
                },
                HeaderValue {
                    name: HeaderName(
                        "Subject",
                    ),
                    raw_value: "subject",
                    encoded_value: "subject",
                },
                HeaderValue {
                    name: HeaderName(
                        "Date",
                    ),
                    raw_value: "Sat, 01 Jan 2000 10:00:00 +0000",
                    encoded_value: "Sat, 01 Jan 2000 10:00:00 +0000",
                },
                HeaderValue {
                    name: HeaderName(
                        "MIME-Version",
                    ),
                    raw_value: "1.0",
                    encoded_value: "1.0",
                },
            ],
        }
        "###);
        assert_debug_snapshot!(message.envelope(), @r###"
        Envelope {
            forward_path: [
                Address {
                    serialized: "dev@secutils.dev",
                    at_start: 3,
                },
            ],
            reverse_path: Some(
                Address {
                    serialized: "smtp@secutils.dev",
                    at_start: 4,
                },
            ),
        }
        "###);

        Ok(())
    }

    #[test]
    fn can_build_transport() -> anyhow::Result<()> {
        assert!(EmailsApi::<&Config>::build_transport(&SmtpConfig {
            username: "smtp@secutils.dev".to_string(),
            password: "changeme".to_string(),
            address: "smtp_server.secutils.dev".to_string(),
            catch_all_recipient: None,
        })
        .is_ok());

        Ok(())
    }
}
