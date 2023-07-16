use crate::{
    api::Api,
    network::{DnsResolver, IpAddrExt, Network},
    users::{PublicUserDataNamespace, User, UserId},
    utils::{
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH,
        web_scraping::{
            resources::{
                web_page_resources_revisions_diff, WebScraperResource, WebScraperResourcesRequest,
                WebScraperResourcesResponse,
            },
            MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY, MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS,
        },
        UtilsWebScrapingActionResult, WebPageResource, WebPageResourcesRevision,
        WebPageResourcesTracker,
    },
};
use anyhow::anyhow;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsWebScrapingAction {
    #[serde(rename_all = "camelCase")]
    FetchWebPageResources {
        tracker_name: String,
        #[serde(default)]
        refresh: bool,
        #[serde(default)]
        calculate_diff: bool,
    },
    #[serde(rename_all = "camelCase")]
    RemoveWebPageResources { tracker_name: String },
    #[serde(rename_all = "camelCase")]
    SaveWebPageResourcesTracker { tracker: WebPageResourcesTracker },
    #[serde(rename_all = "camelCase")]
    RemoveWebPageResourcesTracker { tracker_name: String },
}

impl UtilsWebScrapingAction {
    /// Validates action parameters and throws if action parameters aren't valid.
    pub async fn validate<DR: DnsResolver>(&self, network: &Network<DR>) -> anyhow::Result<()> {
        match self {
            UtilsWebScrapingAction::FetchWebPageResources { tracker_name, .. }
            | UtilsWebScrapingAction::RemoveWebPageResources { tracker_name, .. }
            | UtilsWebScrapingAction::RemoveWebPageResourcesTracker { tracker_name } => {
                if tracker_name.is_empty() {
                    anyhow::bail!("Tracker name cannot be empty");
                }

                if tracker_name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
                    anyhow::bail!(
                        "Tracker name cannot be longer than {} characters",
                        MAX_UTILS_ENTITY_NAME_LENGTH
                    );
                }
            }
            UtilsWebScrapingAction::SaveWebPageResourcesTracker { tracker } => {
                if tracker.name.is_empty() {
                    anyhow::bail!("Tracker name cannot be empty");
                }

                if tracker.name.len() > MAX_UTILS_ENTITY_NAME_LENGTH {
                    anyhow::bail!(
                        "Tracker name cannot be longer than {} characters",
                        MAX_UTILS_ENTITY_NAME_LENGTH
                    );
                }

                if tracker.revisions > MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS {
                    anyhow::bail!(
                        "Tracker revisions count cannot be greater than {}",
                        MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS
                    );
                }

                if tracker.delay > MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY {
                    anyhow::bail!(
                        "Tracker delay cannot be greater than {}ms",
                        MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY.as_millis()
                    );
                }

                if tracker.url.scheme() != "http" && tracker.url.scheme() != "https" {
                    anyhow::bail!("Tracker URL scheme must be either http or https");
                }

                // Checks if the specific hostname is a domain and public (not pointing to the local network).
                let is_public_host_name = if let Some(domain) = tracker.url.domain() {
                    match network.resolver.lookup_ip(domain).await {
                        Ok(lookup) => lookup.iter().all(|ip| IpAddrExt::is_global(&ip)),
                        Err(err) => {
                            log::error!("Cannot resolve `{}` domain to IP: {:?}", domain, err);
                            false
                        }
                    }
                } else {
                    false
                };
                if !is_public_host_name {
                    anyhow::bail!("Tracker URL must have a valid public reachable domain name");
                }
            }
        }

        Ok(())
    }

    pub async fn handle<DR: DnsResolver>(
        self,
        user: User,
        api: &Api,
        network: &Network<DR>,
    ) -> anyhow::Result<UtilsWebScrapingActionResult> {
        match self {
            UtilsWebScrapingAction::SaveWebPageResourcesTracker { tracker } => {
                Ok(UtilsWebScrapingActionResult::SaveWebPageResourcesTracker {
                    tracker: api
                        .web_scraping()
                        .save_web_page_resources_tracker(user.id, tracker)
                        .await?,
                })
            }
            UtilsWebScrapingAction::RemoveWebPageResourcesTracker { tracker_name } => {
                api.web_scraping()
                    .remove_web_page_resources_tracker(user.id, &tracker_name)
                    .await?;
                Ok(UtilsWebScrapingActionResult::RemoveWebPageResourcesTracker)
            }
            UtilsWebScrapingAction::FetchWebPageResources {
                tracker_name,
                refresh,
                calculate_diff,
            } => {
                let tracker = Self::get_tracker(api, user.id, &tracker_name).await?;

                // If tracker is configured to persist resource, and client requests refresh, fetch
                // resources with the scraper and persist them.
                if tracker.revisions > 0 && refresh {
                    // Checks if the specific hostname is a domain and public (not pointing to the local network).
                    let is_public_host_name = if let Some(domain) = tracker.url.domain() {
                        match network.resolver.lookup_ip(domain).await {
                            Ok(lookup) => lookup.iter().all(|ip| IpAddrExt::is_global(&ip)),
                            Err(err) => {
                                log::error!("Cannot resolve `{}` domain to IP: {:?}", domain, err);
                                false
                            }
                        }
                    } else {
                        false
                    };

                    if !is_public_host_name {
                        anyhow::bail!("Tracker URL must have a valid public reachable domain name");
                    }

                    let convert_to_web_page_resources =
                        |resources: Vec<WebScraperResource>| -> Vec<WebPageResource> {
                            resources
                                .into_iter()
                                .map(|resource| resource.into())
                                .collect()
                        };

                    let scraper_response = reqwest::Client::new()
                        .post(format!(
                            "{}api/resources",
                            api.config.components.web_scraper_url.as_str()
                        ))
                        .json(
                            &WebScraperResourcesRequest::with_default_parameters(&tracker.url)
                                .set_delay(tracker.delay),
                        )
                        .send()
                        .await?
                        .json::<WebScraperResourcesResponse>()
                        .await?;

                    api.web_scraping()
                        .save_web_page_resources(
                            user.id,
                            &tracker,
                            WebPageResourcesRevision {
                                timestamp: scraper_response.timestamp,
                                scripts: convert_to_web_page_resources(scraper_response.scripts),
                                styles: convert_to_web_page_resources(scraper_response.styles),
                            },
                        )
                        .await?;
                }

                let revisions = api
                    .web_scraping()
                    .get_web_page_resources(user.id, &tracker)
                    .await?;

                // Retrieve latest persisted resources.
                Ok(UtilsWebScrapingActionResult::FetchWebPageResources {
                    tracker_name,
                    revisions: if calculate_diff {
                        web_page_resources_revisions_diff(revisions)?
                    } else {
                        revisions
                    },
                })
            }
            UtilsWebScrapingAction::RemoveWebPageResources { tracker_name } => {
                api.web_scraping()
                    .remove_tracked_web_page_resources(
                        user.id,
                        &Self::get_tracker(api, user.id, &tracker_name).await?,
                    )
                    .await?;

                Ok(UtilsWebScrapingActionResult::RemoveWebPageResources)
            }
        }
    }

    async fn get_tracker(
        api: &Api,
        user_id: UserId,
        tracker_name: &str,
    ) -> anyhow::Result<WebPageResourcesTracker> {
        api.users()
            .get_data::<BTreeMap<String, WebPageResourcesTracker>>(
                user_id,
                PublicUserDataNamespace::WebPageResourcesTrackers,
            )
            .await?
            .and_then(|mut map| map.value.remove(tracker_name))
            .ok_or_else(|| {
                anyhow!(
                    "Cannot find web page resources tracker with name: {}",
                    tracker_name
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        network::Network,
        tests::MockResolver,
        utils::{UtilsWebScrapingAction, WebPageResourcesTracker},
    };
    use insta::assert_debug_snapshot;
    use std::{net::Ipv4Addr, time::Duration};
    use trust_dns_resolver::{
        proto::rr::{RData, Record},
        Name,
    };
    use url::Url;

    fn mock_network() -> Network<MockResolver> {
        Network::new(MockResolver::new())
    }

    fn mock_network_with_records<const N: usize>(records: Vec<Record>) -> Network<MockResolver<N>> {
        Network::new(MockResolver::new_with_records::<N>(records))
    }

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r###"
    {
        "type": "fetchWebPageResources",
        "value": { "trackerName": "tracker" }
    }
              "###
            )?,
            UtilsWebScrapingAction::FetchWebPageResources {
                tracker_name: "tracker".to_string(),
                refresh: false,
                calculate_diff: false
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r###"
    {
        "type": "fetchWebPageResources",
        "value": { "trackerName": "tracker", "refresh": true, "calculateDiff": true }
    }
              "###
            )?,
            UtilsWebScrapingAction::FetchWebPageResources {
                tracker_name: "tracker".to_string(),
                refresh: true,
                calculate_diff: true
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r###"
    {
        "type": "removeWebPageResources",
        "value": { "trackerName": "tracker" }
    }
              "###
            )?,
            UtilsWebScrapingAction::RemoveWebPageResources {
                tracker_name: "tracker".to_string(),
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r###"
    {
        "type": "saveWebPageResourcesTracker",
        "value": { "tracker": { "name": "some-name", "url": "http://localhost:1234/my/app?q=2", "revisions": 3, "delay": 2000 } }
    }
              "###
            )?,
            UtilsWebScrapingAction::SaveWebPageResourcesTracker {
                tracker: WebPageResourcesTracker {
                    name: "some-name".to_string(),
                    url: Url::parse("http://localhost:1234/my/app?q=2")?,
                    revisions: 3,
                    delay: Duration::from_millis(2000),
                }
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r###"
    {
        "type": "removeWebPageResourcesTracker",
        "value": { "trackerName": "tracker" }
    }
              "###
            )?,
            UtilsWebScrapingAction::RemoveWebPageResourcesTracker {
                tracker_name: "tracker".to_string(),
            }
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn validation() -> anyhow::Result<()> {
        let network = mock_network_with_records::<1>(vec![Record::from_rdata(
            Name::new(),
            300,
            RData::A(Ipv4Addr::new(172, 32, 0, 2)),
        )]);

        assert!(UtilsWebScrapingAction::FetchWebPageResources {
            tracker_name: "a".repeat(100),
            refresh: false,
            calculate_diff: false
        }
        .validate(&mock_network())
        .await
        .is_ok());

        assert!(UtilsWebScrapingAction::RemoveWebPageResources {
            tracker_name: "a".repeat(100),
        }
        .validate(&network)
        .await
        .is_ok());

        assert!(UtilsWebScrapingAction::RemoveWebPageResourcesTracker {
            tracker_name: "a".repeat(100),
        }
        .validate(&network)
        .await
        .is_ok());

        assert!(UtilsWebScrapingAction::FetchWebPageResources {
            tracker_name: "a".repeat(100),
            refresh: false,
            calculate_diff: false
        }
        .validate(&network)
        .await
        .is_ok());

        assert!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "a".repeat(100),
                url: Url::parse("http://google.com/my/app?q=2")?,
                revisions: 10,
                delay: Duration::from_millis(60000),
            }
        }
        .validate(&network)
        .await
        .is_ok());

        assert!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "a".repeat(100),
                url: Url::parse("http://google.com/my/app?q=2")?,
                revisions: 0,
                delay: Duration::from_millis(0),
            }
        }
        .validate(&network)
        .await
        .is_ok());

        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "a".repeat(100),
                url: Url::parse("ftp://google.com/my/app?q=2")?,
                revisions: 0,
                delay: Duration::from_millis(0),
            }
        }
            .validate(&network)
            .await, @r###"
        Err(
            "Tracker URL scheme must be either http or https",
        )
        "###);

        let network_with_local = mock_network_with_records::<1>(vec![Record::from_rdata(
            Name::new(),
            300,
            RData::A(Ipv4Addr::new(127, 0, 0, 1)),
        )]);
        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "a".repeat(100),
                url: Url::parse("http://localhost/my/app?q=2")?,
                revisions: 0,
                delay: Duration::from_millis(0),
            }
        }
            .validate(&network_with_local)
            .await, @r###"
        Err(
            "Tracker URL must have a valid public reachable domain name",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::FetchWebPageResources {
            tracker_name: "".to_string(),
            refresh: false,
            calculate_diff: false
        }
        .validate(&network).await, @r###"
        Err(
            "Tracker name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::FetchWebPageResources {
            tracker_name: "a".repeat(101),
            refresh: false,
            calculate_diff: false
        }
        .validate(&network).await, @r###"
        Err(
            "Tracker name cannot be longer than 100 characters",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::RemoveWebPageResources {
            tracker_name: "".to_string(),
        }
        .validate(&network).await, @r###"
        Err(
            "Tracker name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::RemoveWebPageResources {
            tracker_name: "a".repeat(101),
        }
        .validate(&network).await, @r###"
        Err(
            "Tracker name cannot be longer than 100 characters",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::RemoveWebPageResourcesTracker {
            tracker_name: "".to_string(),
        }
        .validate(&network).await, @r###"
        Err(
            "Tracker name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::RemoveWebPageResourcesTracker {
            tracker_name: "a".repeat(101),
        }
        .validate(&network).await, @r###"
        Err(
            "Tracker name cannot be longer than 100 characters",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "".to_string(),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                revisions: 3,
                delay: Duration::from_millis(2000),
            }
        }
        .validate(&network).await, @r###"
        Err(
            "Tracker name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "a".repeat(101),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                revisions: 3,
                delay: Duration::from_millis(2000),
            }
        }
        .validate(&network).await, @r###"
        Err(
            "Tracker name cannot be longer than 100 characters",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "a".repeat(100),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                revisions: 11,
                delay: Duration::from_millis(2000),
            }
        }
        .validate(&network).await, @r###"
        Err(
            "Tracker revisions count cannot be greater than 10",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker: WebPageResourcesTracker {
                name: "a".repeat(100),
                url: Url::parse("http://localhost:1234/my/app?q=2")?,
                revisions: 10,
                delay: Duration::from_millis(60001),
            }
        }
        .validate(&network).await, @r###"
        Err(
            "Tracker delay cannot be greater than 60000ms",
        )
        "###);

        Ok(())
    }
}
