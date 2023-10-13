use crate::{
    api::Api,
    network::{DnsResolver, EmailTransport},
    users::User,
    utils::{
        utils_action_validation::MAX_UTILS_ENTITY_NAME_LENGTH,
        web_scraping::{
            resources::web_page_resources_revisions_diff, MAX_WEB_PAGE_RESOURCES_TRACKER_DELAY,
            MAX_WEB_PAGE_RESOURCES_TRACKER_REVISIONS,
        },
        UtilsWebScrapingActionResult, WebPageResourcesTracker,
    },
};
use anyhow::anyhow;
use cron::Schedule;
use humantime::format_duration;
use serde::Deserialize;
use std::time::Duration;

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
    pub async fn validate<DR: DnsResolver, ET: EmailTransport>(
        &self,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<()> {
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

                if let Some(ref resource_filter) = tracker.scripts.resource_filter_map {
                    if resource_filter.is_empty() {
                        anyhow::bail!("Tracker resource filter script cannot be empty");
                    }
                }

                if !api.network.is_public_web_url(&tracker.url).await {
                    log::error!(
                        "Tracker URL must be either `http` or `https` and have a valid public reachable domain name: {}",
                        tracker.url
                    );
                    anyhow::bail!(
                        "Tracker URL must be either `http` or `https` and have a valid public reachable domain name"
                    );
                }

                if let Some(schedule) = &tracker.schedule {
                    // Validate that the schedule is a valid cron expression.
                    let schedule = match Schedule::try_from(schedule.as_str()) {
                        Ok(schedule) => schedule,
                        Err(err) => {
                            log::error!("Failed to parse schedule `{}`: {:?}", schedule, err);
                            anyhow::bail!("Tracker schedule must be a valid cron expression");
                        }
                    };

                    // Check if the interval between 10 next occurrences is at least 1 hour.
                    let next_occurrences =
                        schedule.upcoming(chrono::Utc).take(10).collect::<Vec<_>>();
                    let minimum_interval = Duration::from_secs(60 * 60);
                    for (index, occurrence) in next_occurrences.iter().enumerate().skip(1) {
                        let interval = (*occurrence - next_occurrences[index - 1]).to_std()?;
                        if interval < minimum_interval {
                            anyhow::bail!(
                                "Tracker schedule must have at least {} between occurrences, detected {}", 
                                format_duration(minimum_interval),
                                format_duration(interval)
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn handle<DR: DnsResolver, ET: EmailTransport>(
        self,
        user: User,
        api: &Api<DR, ET>,
    ) -> anyhow::Result<UtilsWebScrapingActionResult> {
        match self {
            UtilsWebScrapingAction::SaveWebPageResourcesTracker { tracker } => {
                Ok(UtilsWebScrapingActionResult::SaveWebPageResourcesTracker {
                    tracker: api
                        .web_scraping()
                        .upsert_resources_tracker(user.id, tracker)
                        .await?,
                })
            }
            UtilsWebScrapingAction::RemoveWebPageResourcesTracker { tracker_name } => {
                api.web_scraping()
                    .remove_resources_tracker(user.id, &tracker_name)
                    .await?;
                Ok(UtilsWebScrapingActionResult::RemoveWebPageResourcesTracker)
            }
            UtilsWebScrapingAction::FetchWebPageResources {
                tracker_name,
                refresh,
                calculate_diff,
            } => {
                let web_scraping = api.web_scraping();
                let tracker = web_scraping
                    .get_resources_tracker(user.id, &tracker_name)
                    .await?
                    .ok_or_else(|| {
                        anyhow!(
                            "Cannot find web page resources tracker with name: {}",
                            tracker_name
                        )
                    })?;

                // If tracker is configured to persist resource, and client requests refresh, fetch
                // resources with the scraper and persist them.
                if tracker.revisions > 0 && refresh {
                    web_scraping
                        .save_resources(
                            user.id,
                            &tracker,
                            web_scraping.fetch_resources(&tracker).await?,
                        )
                        .await?;
                }

                let revisions = web_scraping.get_resources(user.id, &tracker).await?;

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
                let web_scraping = api.web_scraping();
                let tracker = web_scraping
                    .get_resources_tracker(user.id, &tracker_name)
                    .await?
                    .ok_or_else(|| {
                        anyhow!(
                            "Cannot find web page resources tracker with name: {}",
                            tracker_name
                        )
                    })?;
                web_scraping
                    .remove_tracked_resources(user.id, &tracker)
                    .await?;

                Ok(UtilsWebScrapingActionResult::RemoveWebPageResources)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        tests::{
            mock_api, mock_api_with_network, mock_network_with_records,
            MockWebPageResourcesTrackerBuilder,
        },
        utils::{UtilsWebScrapingAction, WebPageResourcesTrackerScripts},
    };
    use insta::assert_debug_snapshot;
    use std::net::Ipv4Addr;
    use trust_dns_resolver::{
        proto::rr::{rdata::A, RData, Record},
        Name,
    };

    #[test]
    fn deserialization() -> anyhow::Result<()> {
        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r#"
    {
        "type": "fetchWebPageResources",
        "value": { "trackerName": "tracker" }
    }
              "#
            )?,
            UtilsWebScrapingAction::FetchWebPageResources {
                tracker_name: "tracker".to_string(),
                refresh: false,
                calculate_diff: false
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r#"
    {
        "type": "fetchWebPageResources",
        "value": { "trackerName": "tracker", "refresh": true, "calculateDiff": true }
    }
              "#
            )?,
            UtilsWebScrapingAction::FetchWebPageResources {
                tracker_name: "tracker".to_string(),
                refresh: true,
                calculate_diff: true
            }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r#"
    {
        "type": "removeWebPageResources",
        "value": { "trackerName": "tracker" }
    }
              "#
            )?,
            UtilsWebScrapingAction::RemoveWebPageResources {
                tracker_name: "tracker".to_string(),
            }
        );

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r#"
    {
        "type": "saveWebPageResourcesTracker",
        "value": { "tracker": { "name": "some-name", "url": "http://localhost:1234/my/app?q=2", "revisions": 3, "delay": 2000 } }
    }
              "#
            )?,
            UtilsWebScrapingAction::SaveWebPageResourcesTracker { tracker }
        );

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "some-name",
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .with_schedule("0 0 * * * *")
        .build();
        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r#"
    {
        "type": "saveWebPageResourcesTracker",
        "value": { "tracker": { "name": "some-name", "url": "http://localhost:1234/my/app?q=2", "revisions": 3, "delay": 2000, "schedule": "0 0 * * * *" } }
    }
              "#
            )?,
            UtilsWebScrapingAction::SaveWebPageResourcesTracker { tracker }
        );

        assert_eq!(
            serde_json::from_str::<UtilsWebScrapingAction>(
                r#"
    {
        "type": "removeWebPageResourcesTracker",
        "value": { "trackerName": "tracker" }
    }
              "#
            )?,
            UtilsWebScrapingAction::RemoveWebPageResourcesTracker {
                tracker_name: "tracker".to_string(),
            }
        );

        Ok(())
    }

    #[actix_rt::test]
    async fn validation() -> anyhow::Result<()> {
        let api = mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
            Name::new(),
            300,
            RData::A(A(Ipv4Addr::new(172, 32, 0, 2))),
        )]))
        .await?;

        assert!(UtilsWebScrapingAction::FetchWebPageResources {
            tracker_name: "a".repeat(100),
            refresh: false,
            calculate_diff: false
        }
        .validate(&mock_api().await?)
        .await
        .is_ok());

        assert!(UtilsWebScrapingAction::RemoveWebPageResources {
            tracker_name: "a".repeat(100),
        }
        .validate(&api)
        .await
        .is_ok());

        assert!(UtilsWebScrapingAction::RemoveWebPageResourcesTracker {
            tracker_name: "a".repeat(100),
        }
        .validate(&api)
        .await
        .is_ok());

        assert!(UtilsWebScrapingAction::FetchWebPageResources {
            tracker_name: "a".repeat(100),
            refresh: false,
            calculate_diff: false
        }
        .validate(&api)
        .await
        .is_ok());

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "a".repeat(100),
            "http://google.com/my/app?q=2",
            10,
        )?
        .with_delay_millis(60000)
        .build();
        assert!(
            UtilsWebScrapingAction::SaveWebPageResourcesTracker { tracker }
                .validate(&api)
                .await
                .is_ok()
        );

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "a".repeat(100),
            "http://google.com/my/app?q=2",
            0,
        )?
        .with_delay_millis(0)
        .build();
        assert!(
            UtilsWebScrapingAction::SaveWebPageResourcesTracker { tracker }
                .validate(&api)
                .await
                .is_ok()
        );

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "a".repeat(100),
            "http://google.com/my/app?q=2",
            10,
        )?
        .with_delay_millis(0)
        .with_schedule("0 0 0 * * *")
        .build();
        assert!(
            UtilsWebScrapingAction::SaveWebPageResourcesTracker { tracker }
                .validate(&api)
                .await
                .is_ok()
        );

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "a".repeat(100),
            "ftp://google.com/my/app?q=2",
            0,
        )?
        .with_delay_millis(0)
        .build();
        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker
        }
            .validate(&api)
            .await, @r###"
        Err(
            "Tracker URL must be either `http` or `https` and have a valid public reachable domain name",
        )
        "###);

        let api_with_local_network =
            mock_api_with_network(mock_network_with_records::<1>(vec![Record::from_rdata(
                Name::new(),
                300,
                RData::A(A(Ipv4Addr::new(127, 0, 0, 1))),
            )]))
            .await?;

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "a".repeat(100),
            "http://google.com/my/app?q=2",
            0,
        )?
        .with_delay_millis(0)
        .build();
        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker
        }
            .validate(&api_with_local_network)
            .await, @r###"
        Err(
            "Tracker URL must be either `http` or `https` and have a valid public reachable domain name",
        )
        "###);

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "a".repeat(100),
            "http://google.com/my/app?q=2",
            0,
        )?
        .with_delay_millis(0)
        .with_schedule("0 * * * * *")
        .build();
        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker
        }
        .validate(&api)
        .await, @r###"
        Err(
            "Tracker schedule must have at least 1h between occurrences, detected 1m",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::FetchWebPageResources {
            tracker_name: "".to_string(),
            refresh: false,
            calculate_diff: false
        }
        .validate(&api).await, @r###"
        Err(
            "Tracker name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::FetchWebPageResources {
            tracker_name: "a".repeat(101),
            refresh: false,
            calculate_diff: false
        }
        .validate(&api).await, @r###"
        Err(
            "Tracker name cannot be longer than 100 characters",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::RemoveWebPageResources {
            tracker_name: "".to_string(),
        }
        .validate(&api).await, @r###"
        Err(
            "Tracker name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::RemoveWebPageResources {
            tracker_name: "a".repeat(101),
        }
        .validate(&api).await, @r###"
        Err(
            "Tracker name cannot be longer than 100 characters",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::RemoveWebPageResourcesTracker {
            tracker_name: "".to_string(),
        }
        .validate(&api).await, @r###"
        Err(
            "Tracker name cannot be empty",
        )
        "###);

        assert_debug_snapshot!(UtilsWebScrapingAction::RemoveWebPageResourcesTracker {
            tracker_name: "a".repeat(101),
        }
        .validate(&api).await, @r###"
        Err(
            "Tracker name cannot be longer than 100 characters",
        )
        "###);

        let tracker =
            MockWebPageResourcesTrackerBuilder::create("", "http://localhost:1234/my/app?q=2", 3)?
                .build();
        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker
        }
        .validate(&api).await, @r###"
        Err(
            "Tracker name cannot be empty",
        )
        "###);

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "a".repeat(101),
            "http://localhost:1234/my/app?q=2",
            3,
        )?
        .build();
        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker
        }
        .validate(&api).await, @r###"
        Err(
            "Tracker name cannot be longer than 100 characters",
        )
        "###);

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "a".repeat(100),
            "http://localhost:1234/my/app?q=2",
            11,
        )?
        .build();
        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker
        }
        .validate(&api).await, @r###"
        Err(
            "Tracker revisions count cannot be greater than 10",
        )
        "###);

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "a".repeat(100),
            "http://localhost:1234/my/app?q=2",
            10,
        )?
        .with_delay_millis(60001)
        .build();
        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker
        }
        .validate(&api).await, @r###"
        Err(
            "Tracker delay cannot be greater than 60000ms",
        )
        "###);

        let tracker = MockWebPageResourcesTrackerBuilder::create(
            "a".repeat(100),
            "http://localhost:1234/my/app?q=2",
            10,
        )?
        .with_scripts(WebPageResourcesTrackerScripts {
            resource_filter_map: Some("".to_string()),
        })
        .build();
        assert_debug_snapshot!(UtilsWebScrapingAction::SaveWebPageResourcesTracker {
            tracker
        }
        .validate(&api).await, @r###"
        Err(
            "Tracker resource filter script cannot be empty",
        )
        "###);

        Ok(())
    }
}
