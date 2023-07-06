use crate::utils::{
    UtilsCertificatesAction, UtilsWebScrapingAction, UtilsWebSecurityAction, UtilsWebhooksAction,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsAction {
    Certificates(UtilsCertificatesAction),
    Webhooks(UtilsWebhooksAction),
    WebScraping(UtilsWebScrapingAction),
    WebSecurity(UtilsWebSecurityAction),
}

impl UtilsAction {
    /// Validates action parameters and throws if action parameters aren't valid.
    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            UtilsAction::Certificates(action) => action.validate(),
            UtilsAction::Webhooks(action) => action.validate(),
            UtilsAction::WebScraping(action) => action.validate(),
            UtilsAction::WebSecurity(action) => action.validate(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::{
        CertificateFormat, ContentSecurityPolicySource, UtilsAction, UtilsCertificatesAction,
        UtilsWebScrapingAction, UtilsWebSecurityAction, UtilsWebhooksAction,
        WebPageResourcesTracker,
    };
    use insta::assert_debug_snapshot;
    use std::time::Duration;
    use url::Url;

    #[test]
    fn validation_certificates() -> anyhow::Result<()> {
        assert!(UtilsAction::Certificates(
            UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_name: "a".repeat(100),
                format: CertificateFormat::Pem,
                passphrase: None,
            }
        )
        .validate()
        .is_ok());

        assert_debug_snapshot!(UtilsAction::Certificates(UtilsCertificatesAction::GenerateSelfSignedCertificate {
            template_name: "".to_string(),
            format: CertificateFormat::Pem,
            passphrase: None,
        }).validate(), @r###"
        Err(
            "Template name cannot be empty",
        )
        "###);

        Ok(())
    }

    #[test]
    fn validation_webhooks() -> anyhow::Result<()> {
        assert!(
            UtilsAction::Webhooks(UtilsWebhooksAction::GetAutoRespondersRequests {
                auto_responder_name: "a".repeat(100),
            })
            .validate()
            .is_ok()
        );

        assert_debug_snapshot!(UtilsAction::Webhooks(UtilsWebhooksAction::GetAutoRespondersRequests {
            auto_responder_name: "".to_string(),
        })
        .validate(), @r###"
        Err(
            "Auto responder name cannot be empty",
        )
        "###);

        Ok(())
    }

    #[test]
    fn validation_web_scraping() -> anyhow::Result<()> {
        assert!(
            UtilsAction::WebScraping(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
                tracker: WebPageResourcesTracker {
                    name: "a".repeat(100),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 0,
                    delay: Duration::from_millis(0),
                }
            })
            .validate()
            .is_ok()
        );

        assert_debug_snapshot!(UtilsAction::WebScraping(UtilsWebScrapingAction::FetchWebPageResources {
            tracker_name: "".to_string(),
            refresh: false,
            calculate_diff: false
        })
        .validate(), @r###"
        Err(
            "Tracker name cannot be empty",
        )
        "###);

        Ok(())
    }

    #[test]
    fn validation_web_security() -> anyhow::Result<()> {
        assert!(
            UtilsAction::WebSecurity(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                policy_name: "a".repeat(100),
                source: ContentSecurityPolicySource::Meta,
            })
            .validate()
            .is_ok()
        );

        assert_debug_snapshot!( UtilsAction::WebSecurity(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
            policy_name: "".to_string(),
            source: ContentSecurityPolicySource::Meta,
        })
        .validate(), @r###"
        Err(
            "Policy name cannot be empty",
        )
        "###);

        Ok(())
    }
}
