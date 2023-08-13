use crate::{
    api::Api,
    network::DnsResolver,
    users::User,
    utils::{
        UtilsActionResult, UtilsCertificatesAction, UtilsWebScrapingAction, UtilsWebSecurityAction,
        UtilsWebhooksAction,
    },
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
    pub async fn validate<DR: DnsResolver>(&self, api: &Api<DR>) -> anyhow::Result<()> {
        match self {
            UtilsAction::Certificates(action) => action.validate(),
            UtilsAction::Webhooks(action) => action.validate(),
            UtilsAction::WebScraping(action) => action.validate(api).await,
            UtilsAction::WebSecurity(action) => action.validate(),
        }
    }

    /// Consumes and handles action.
    pub async fn handle<DR: DnsResolver>(
        self,
        user: User,
        api: &Api<DR>,
    ) -> anyhow::Result<UtilsActionResult> {
        match self {
            UtilsAction::Certificates(action) => action
                .handle(user, api)
                .await
                .map(UtilsActionResult::Certificates),
            UtilsAction::Webhooks(action) => action
                .handle(user, api)
                .await
                .map(UtilsActionResult::Webhooks),
            UtilsAction::WebScraping(action) => action
                .handle(user, api)
                .await
                .map(UtilsActionResult::WebScraping),
            UtilsAction::WebSecurity(action) => action
                .handle(user, api)
                .await
                .map(UtilsActionResult::WebSecurity),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        network::Network,
        tests::{mock_api, mock_api_with_network, MockResolver},
        utils::{
            CertificateFormat, ContentSecurityPolicySource, UtilsAction, UtilsCertificatesAction,
            UtilsWebScrapingAction, UtilsWebSecurityAction, UtilsWebhooksAction,
            WebPageResourcesTracker,
        },
    };
    use insta::assert_debug_snapshot;
    use std::{net::Ipv4Addr, time::Duration};
    use trust_dns_resolver::{
        proto::rr::{RData, Record},
        Name,
    };
    use url::Url;

    fn mock_network_with_records<const N: usize>(records: Vec<Record>) -> Network<MockResolver<N>> {
        Network::new(MockResolver::new_with_records::<N>(records))
    }

    #[actix_rt::test]
    async fn validation_certificates() -> anyhow::Result<()> {
        assert!(UtilsAction::Certificates(
            UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_name: "a".repeat(100),
                format: CertificateFormat::Pem,
                passphrase: None,
            }
        )
        .validate(&mock_api().await?)
        .await
        .is_ok());

        assert_debug_snapshot!(UtilsAction::Certificates(UtilsCertificatesAction::GenerateSelfSignedCertificate {
            template_name: "".to_string(),
            format: CertificateFormat::Pem,
            passphrase: None,
        }).validate(&mock_api().await?).await, @r###"
        Err(
            "Template name cannot be empty",
        )
        "###);

        Ok(())
    }

    #[actix_rt::test]
    async fn validation_webhooks() -> anyhow::Result<()> {
        assert!(
            UtilsAction::Webhooks(UtilsWebhooksAction::GetAutoRespondersRequests {
                auto_responder_name: "a".repeat(100),
            })
            .validate(&mock_api().await?)
            .await
            .is_ok()
        );

        assert_debug_snapshot!(UtilsAction::Webhooks(UtilsWebhooksAction::GetAutoRespondersRequests {
            auto_responder_name: "".to_string(),
        })
        .validate(&mock_api().await?).await, @r###"
        Err(
            "Auto responder name cannot be empty",
        )
        "###);

        Ok(())
    }

    #[actix_rt::test]
    async fn validation_web_scraping() -> anyhow::Result<()> {
        assert!(
            UtilsAction::WebScraping(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
                tracker: WebPageResourcesTracker {
                    name: "a".repeat(100),
                    url: Url::parse("http://google.com/my/app?q=2")?,
                    revisions: 0,
                    delay: Duration::from_millis(0),
                    schedule: Some("0 0 0 1 * *".to_string()),
                }
            })
            .validate(
                &mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                    Name::new(),
                    300,
                    RData::A(Ipv4Addr::new(172, 32, 0, 2)),
                )]))
                .await?
            )
            .await
            .is_ok()
        );

        assert_debug_snapshot!(UtilsAction::WebScraping(UtilsWebScrapingAction::FetchWebPageResources {
            tracker_name: "".to_string(),
            refresh: false,
            calculate_diff: false
        })
        .validate(&mock_api().await?).await, @r###"
        Err(
            "Tracker name cannot be empty",
        )
        "###);

        Ok(())
    }

    #[actix_rt::test]
    async fn validation_web_security() -> anyhow::Result<()> {
        assert!(
            UtilsAction::WebSecurity(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                policy_name: "a".repeat(100),
                source: ContentSecurityPolicySource::Meta,
            })
            .validate(&mock_api().await?)
            .await
            .is_ok()
        );

        assert_debug_snapshot!( UtilsAction::WebSecurity(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
            policy_name: "".to_string(),
            source: ContentSecurityPolicySource::Meta,
        })
        .validate(&mock_api().await?).await, @r###"
        Err(
            "Policy name cannot be empty",
        )
        "###);

        Ok(())
    }
}
