use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::User,
    utils::{
        UtilsLegacyActionResult, UtilsWebScrapingAction, UtilsWebSecurityAction,
        UtilsWebhooksAction,
    },
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsLegacyAction {
    Webhooks(UtilsWebhooksAction),
    WebScraping(UtilsWebScrapingAction),
    WebSecurity(UtilsWebSecurityAction),
}

impl UtilsLegacyAction {
    /// Validates action parameters and throws if action parameters aren't valid.
    pub async fn validate<DR: DnsResolver, ET: EmailTransport>(
        &self,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<()> {
        match self {
            UtilsLegacyAction::Webhooks(action) => action.validate(),
            UtilsLegacyAction::WebScraping(action) => action.validate(api).await,
            UtilsLegacyAction::WebSecurity(action) => action.validate(api).await,
        }
    }

    /// Consumes and handles action.
    pub async fn handle<DR: DnsResolver, ET: EmailTransport>(
        self,
        user: User,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<UtilsLegacyActionResult> {
        match self {
            UtilsLegacyAction::Webhooks(action) => action
                .handle(user, api)
                .await
                .map(UtilsLegacyActionResult::Webhooks),
            UtilsLegacyAction::WebScraping(action) => action
                .handle(user, api)
                .await
                .map(UtilsLegacyActionResult::WebScraping),
            UtilsLegacyAction::WebSecurity(action) => action
                .handle(user, api)
                .await
                .map(UtilsLegacyActionResult::WebSecurity),
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
            AutoResponder, AutoResponderMethod, ContentSecurityPolicySource, UtilsLegacyAction,
            UtilsWebScrapingAction, UtilsWebSecurityAction, UtilsWebhooksAction,
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
    async fn validation_webhooks() -> anyhow::Result<()> {
        assert!(
            UtilsLegacyAction::Webhooks(UtilsWebhooksAction::SaveAutoResponder {
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

        assert_debug_snapshot!(UtilsLegacyAction::Webhooks(UtilsWebhooksAction::SaveAutoResponder {
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
            UtilsLegacyAction::Webhooks(UtilsWebhooksAction::RemoveAutoResponder {
                responder_path: "/a".repeat(50),
            })
            .validate(&mock_api().await?)
            .await
            .is_ok()
        );

        assert_debug_snapshot!(UtilsLegacyAction::Webhooks(UtilsWebhooksAction::RemoveAutoResponder {
            responder_path: "a".to_string(),
        })
        .validate(&mock_api().await?).await, @r###"
        Err(
            "Auto responder path is not valid.",
        )
        "###);

        assert!(
            UtilsLegacyAction::Webhooks(UtilsWebhooksAction::GetAutoRespondersRequests {
                responder_path: "/a".repeat(50),
            })
            .validate(&mock_api().await?)
            .await
            .is_ok()
        );

        assert_debug_snapshot!(UtilsLegacyAction::Webhooks(UtilsWebhooksAction::GetAutoRespondersRequests {
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
        assert!(UtilsLegacyAction::WebScraping(
            UtilsWebScrapingAction::SaveWebPageResourcesTracker { tracker }
        )
        .validate(
            &mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(172, 32, 0, 2))),
            )]))
            .await?
        )
        .await
        .is_ok());

        assert_debug_snapshot!(UtilsLegacyAction::WebScraping(UtilsWebScrapingAction::FetchWebPageResources {
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
        assert!(UtilsLegacyAction::WebSecurity(
            UtilsWebSecurityAction::SerializeContentSecurityPolicy {
                policy_name: "a".repeat(100),
                source: ContentSecurityPolicySource::Meta,
            }
        )
        .validate(&mock_api().await?)
        .await
        .is_ok());

        assert_debug_snapshot!(UtilsLegacyAction::WebSecurity(UtilsWebSecurityAction::SerializeContentSecurityPolicy {
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
