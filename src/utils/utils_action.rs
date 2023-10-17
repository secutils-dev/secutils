use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
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
    pub async fn validate<DR: DnsResolver, ET: EmailTransport>(
        &self,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<()> {
        match self {
            UtilsAction::Certificates(action) => action.validate(),
            UtilsAction::Webhooks(action) => action.validate(),
            UtilsAction::WebScraping(action) => action.validate(api).await,
            UtilsAction::WebSecurity(action) => action.validate(api).await,
        }
    }

    /// Consumes and handles action.
    pub async fn handle<DR: DnsResolver, ET: EmailTransport>(
        self,
        user: User,
        api: &Api<DR, ET>,
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
        tests::{
            mock_api, mock_api_with_network, MockResolver, MockWebPageResourcesTrackerBuilder,
        },
        utils::{
            AutoResponder, AutoResponderMethod, ContentSecurityPolicySource, ExportFormat,
            UtilsAction, UtilsCertificatesAction, UtilsWebScrapingAction, UtilsWebSecurityAction,
            UtilsWebhooksAction,
        },
    };
    use insta::assert_debug_snapshot;
    use lettre::transport::stub::AsyncStubTransport;
    use std::net::Ipv4Addr;
    use trust_dns_resolver::{
        proto::rr::{rdata::A, RData, Record},
        Name,
    };

    fn mock_network_with_records<const N: usize>(
        records: Vec<Record>,
    ) -> Network<MockResolver<N>, AsyncStubTransport> {
        Network::new(
            MockResolver::new_with_records::<N>(records),
            AsyncStubTransport::new_ok(),
        )
    }

    #[actix_rt::test]
    async fn validation_certificates() -> anyhow::Result<()> {
        assert!(UtilsAction::Certificates(
            UtilsCertificatesAction::GenerateSelfSignedCertificate {
                template_name: "a".repeat(100),
                format: ExportFormat::Pem,
                passphrase: None,
            }
        )
        .validate(&mock_api().await?)
        .await
        .is_ok());

        assert_debug_snapshot!(UtilsAction::Certificates(UtilsCertificatesAction::GenerateSelfSignedCertificate {
            template_name: "".to_string(),
            format: ExportFormat::Pem,
            passphrase: None,
        }).validate(&mock_api().await?).await, @r###"
        Err(
            "Certificate template name cannot be empty.",
        )
        "###);

        Ok(())
    }

    #[actix_rt::test]
    async fn validation_webhooks() -> anyhow::Result<()> {
        assert!(
            UtilsAction::Webhooks(UtilsWebhooksAction::SaveAutoResponder {
                responder: AutoResponder {
                    path: "/name".to_string(),
                    method: AutoResponderMethod::Post,
                    requests_to_track: 3,
                    status_code: 200,
                    body: None,
                    headers: Some(vec![("key".to_string(), "value".to_string())]),
                    delay: None,
                }
            })
            .validate(&mock_api().await?)
            .await
            .is_ok()
        );

        assert_debug_snapshot!(UtilsAction::Webhooks(UtilsWebhooksAction::SaveAutoResponder {
            responder: AutoResponder {
                path: "/name".to_string(),
                method: AutoResponderMethod::Post,
                requests_to_track: 3,
                status_code: 2000,
                body: None,
                headers: Some(vec![("key".to_string(), "value".to_string())]),
                delay: None,
            }
        })
        .validate(&mock_api().await?).await, @r###"
        Err(
            "Auto responder is not valid.",
        )
        "###);

        assert!(
            UtilsAction::Webhooks(UtilsWebhooksAction::RemoveAutoResponder {
                responder_path: "/a".repeat(50),
            })
            .validate(&mock_api().await?)
            .await
            .is_ok()
        );

        assert_debug_snapshot!(UtilsAction::Webhooks(UtilsWebhooksAction::RemoveAutoResponder {
            responder_path: "a".to_string(),
        })
        .validate(&mock_api().await?).await, @r###"
        Err(
            "Auto responder path is not valid.",
        )
        "###);

        assert!(
            UtilsAction::Webhooks(UtilsWebhooksAction::GetAutoRespondersRequests {
                responder_path: "/a".repeat(50),
            })
            .validate(&mock_api().await?)
            .await
            .is_ok()
        );

        assert_debug_snapshot!(UtilsAction::Webhooks(UtilsWebhooksAction::GetAutoRespondersRequests {
            responder_path: "a".to_string(),
        })
        .validate(&mock_api().await?).await, @r###"
        Err(
            "Auto responder path is not valid.",
        )
        "###);

        Ok(())
    }

    #[actix_rt::test]
    async fn validation_web_scraping() -> anyhow::Result<()> {
        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "a".repeat(100),
            "http://google.com/my/app?q=2",
            0,
        )?
        .with_schedule("0 0 0 1 * *")
        .with_delay_millis(0)
        .build();
        assert!(
            UtilsAction::WebScraping(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
                tracker
            })
            .validate(
                &mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                    Name::new(),
                    300,
                    RData::A(A(Ipv4Addr::new(172, 32, 0, 2))),
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
            "Tracker name cannot be empty.",
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
            "Policy name cannot be empty.",
        )
        "###);

        Ok(())
    }
}
