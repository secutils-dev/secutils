mod webauthn_challenge;
mod webauthn_challenge_type;
mod webauthn_session;
mod webauthn_session_value;

use crate::config::Config;
use anyhow::{anyhow, Context};
use webauthn_rs::{Webauthn, WebauthnBuilder};

pub use self::{
    webauthn_challenge::WebAuthnChallenge, webauthn_challenge_type::WebAuthnChallengeType,
    webauthn_session::WebAuthnSession, webauthn_session_value::WebAuthnSessionValue,
};

/// Name of the session dictionary key used to store intermediate WebAuthn registration and
/// authentication states.
pub const WEBAUTHN_SESSION_KEY: &str = "webauthn_session";

pub fn create_webauthn(config: &Config) -> anyhow::Result<Webauthn> {
    let rp_id = config
        .public_url
        .host_str()
        .ok_or_else(|| anyhow!("Public URL doesn't contain valid host name."))?;

    let builder = WebauthnBuilder::new(rp_id, &config.public_url)
        .with_context(|| "Invalid WebAuthn configuration.".to_string())?;
    builder
        .rp_name("Secutils.dev")
        .build()
        .with_context(|| "Failed to build WebAuthn.")
}

#[cfg(test)]
mod tests {
    use crate::{
        authentication::create_webauthn,
        config::{ComponentsConfig, Config},
    };
    use url::Url;

    #[test]
    fn can_create_webauthn() -> anyhow::Result<()> {
        let config = Config {
            version: "1.0.0".to_string(),
            http_port: 1234,
            public_url: Url::parse("http://localhost:1234")?,
            smtp: None,
            components: ComponentsConfig {
                web_scraper_url: Url::parse("http://localhost:7272")?,
                search_index_version: 1,
            },
        };

        let webauthn = create_webauthn(&config)?;
        assert_eq!(
            webauthn
                .get_allowed_origins()
                .iter()
                .map(|url| url.to_string())
                .collect::<Vec<_>>(),
            vec!["http://localhost:1234/".to_string()]
        );

        Ok(())
    }
}
