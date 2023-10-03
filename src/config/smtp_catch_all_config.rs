use regex::Regex;

/// Configuration for the SMTP catch-all functionality.
#[derive(Clone, Debug)]
pub struct SmtpCatchAllConfig {
    /// Address of the catch-all email recipient.
    pub recipient: String,
    /// Email will be sent to the catch-all recipient instead of original one only if the email text
    /// matches regular expression specified in `text_matcher`.
    pub text_matcher: Regex,
}
