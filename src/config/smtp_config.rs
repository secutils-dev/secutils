/// Configuration for the SMTP functionality.
#[derive(Clone, Debug)]
pub struct SmtpConfig {
    /// Username to use to authenticate to the SMTP server.
    pub username: String,
    /// Password to use to authenticate to the SMTP server.
    pub password: String,
    /// Address of the SMTP server.
    pub address: String,
    /// Address of the email recipient (used for debug only).
    pub catch_all_recipient: Option<String>,
}
