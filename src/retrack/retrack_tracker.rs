use crate::retrack::tags::{RETRACK_NOTIFICATIONS_TAG, get_tag_value};
use retrack_types::trackers::{Tracker, TrackerConfig, TrackerTarget};
use serde::{Serialize, Serializer};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged, rename_all = "camelCase")]
pub enum RetrackTracker {
    Reference { id: Uuid },
    Value(Box<RetrackTrackerValue>),
}

impl RetrackTracker {
    pub fn id(&self) -> Uuid {
        match self {
            RetrackTracker::Reference { id } => *id,
            RetrackTracker::Value(value) => value.id,
        }
    }

    /// Creates retrack tracker "view" from the given tracker (by value).
    pub fn from_value(tracker: Tracker) -> Self {
        RetrackTracker::Value(Box::new(RetrackTrackerValue {
            id: tracker.id,
            enabled: tracker.enabled,
            config: tracker.config,
            target: tracker.target,
            notifications: get_tag_value(&tracker.tags, RETRACK_NOTIFICATIONS_TAG)
                .and_then(|tag| tag.parse::<bool>().ok())
                .unwrap_or_default(),
        }))
    }

    /// Creates retrack tracker "view" from the given tracker id (by reference).
    pub fn from_reference(tracker_id: Uuid) -> Self {
        RetrackTracker::Reference { id: tracker_id }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RetrackTrackerValue {
    pub id: Uuid,
    pub enabled: bool,
    pub config: TrackerConfig,
    #[serde(serialize_with = "serialize_tracker_target")]
    pub target: TrackerTarget,
    pub notifications: bool,
}

/// Serializes `TrackerTarget` so that API targets are flattened from the
/// Retrack `{ requests: [{ url, method, … }], configurator, extractor }` form
/// into the Secutils UI form `{ url, method, headers, body, … , configurator, extractor }`.
/// Page targets are serialized as-is (the UI reads fields like `extractor` directly).
fn serialize_tracker_target<S>(target: &TrackerTarget, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match target {
        TrackerTarget::Page(_) => target.serialize(serializer),
        TrackerTarget::Api(api) => {
            let mut map = serde_json::Map::new();
            if let Some(req) = api.requests.first() {
                map.insert("url".into(), req.url.to_string().into());
                if let Some(method) = &req.method {
                    map.insert("method".into(), method.to_string().into());
                }
                if let Some(headers) = &req.headers {
                    let h: serde_json::Map<String, serde_json::Value> = headers
                        .iter()
                        .map(|(k, v)| {
                            (
                                k.to_string(),
                                v.to_str().unwrap_or_default().to_string().into(),
                            )
                        })
                        .collect();
                    map.insert("headers".into(), serde_json::Value::Object(h));
                }
                if let Some(body) = &req.body {
                    map.insert("body".into(), body.clone());
                }
                if let Some(media_type) = &req.media_type {
                    map.insert("mediaType".into(), media_type.to_string().into());
                }
                if let Some(accept_statuses) = &req.accept_statuses {
                    let statuses: Vec<serde_json::Value> =
                        accept_statuses.iter().map(|s| s.as_u16().into()).collect();
                    map.insert("acceptStatuses".into(), serde_json::Value::Array(statuses));
                }
                if req.accept_invalid_certificates {
                    map.insert("acceptInvalidCertificates".into(), true.into());
                }
            }
            if let Some(configurator) = &api.configurator {
                map.insert("configurator".into(), configurator.clone().into());
            }
            if let Some(extractor) = &api.extractor {
                map.insert("extractor".into(), extractor.clone().into());
            }
            serde_json::Value::Object(map).serialize(serializer)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RetrackTrackerValue;
    use insta::assert_json_snapshot;
    use retrack_types::{
        scheduler::SchedulerJobConfig,
        trackers::{ApiTarget, PageTarget, TargetRequest, TrackerConfig, TrackerTarget},
    };
    use serde_json::json;
    use uuid::uuid;

    fn minimal_config() -> TrackerConfig {
        TrackerConfig {
            revisions: 1,
            timeout: None,
            job: Some(SchedulerJobConfig {
                schedule: "@daily".to_string(),
                retry_strategy: None,
            }),
        }
    }

    #[test]
    fn page_target_serializes_as_is() -> anyhow::Result<()> {
        let value = RetrackTrackerValue {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            enabled: true,
            config: minimal_config(),
            target: TrackerTarget::Page(PageTarget {
                extractor: "return document.title;".to_string(),
                params: None,
                engine: None,
                user_agent: None,
                accept_invalid_certificates: false,
            }),
            notifications: false,
        };
        assert_json_snapshot!(value, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "enabled": true,
          "config": {
            "revisions": 1,
            "job": {
              "schedule": "@daily"
            }
          },
          "target": {
            "type": "page",
            "extractor": "return document.title;"
          },
          "notifications": false
        }
        "###);
        Ok(())
    }

    #[test]
    fn page_target_with_accept_invalid_certificates() -> anyhow::Result<()> {
        let value = RetrackTrackerValue {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            enabled: true,
            config: minimal_config(),
            target: TrackerTarget::Page(PageTarget {
                extractor: "return document.title;".to_string(),
                params: None,
                engine: None,
                user_agent: None,
                accept_invalid_certificates: true,
            }),
            notifications: false,
        };
        assert_json_snapshot!(value, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "enabled": true,
          "config": {
            "revisions": 1,
            "job": {
              "schedule": "@daily"
            }
          },
          "target": {
            "type": "page",
            "extractor": "return document.title;",
            "acceptInvalidCertificates": true
          },
          "notifications": false
        }
        "###);
        Ok(())
    }

    #[test]
    fn api_target_minimal_is_flattened() -> anyhow::Result<()> {
        let value = RetrackTrackerValue {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            enabled: true,
            config: minimal_config(),
            target: TrackerTarget::Api(ApiTarget {
                requests: vec![TargetRequest::new("https://api.example.com/data".parse()?)],
                configurator: None,
                extractor: None,
                params: None,
            }),
            notifications: false,
        };
        assert_json_snapshot!(value, @r###"
        {
          "id": "00000000-0000-0000-0000-000000000001",
          "enabled": true,
          "config": {
            "revisions": 1,
            "job": {
              "schedule": "@daily"
            }
          },
          "target": {
            "url": "https://api.example.com/data"
          },
          "notifications": false
        }
        "###);
        Ok(())
    }

    #[test]
    fn api_target_full_is_flattened() -> anyhow::Result<()> {
        let value = RetrackTrackerValue {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            enabled: false,
            config: minimal_config(),
            target: TrackerTarget::Api(ApiTarget {
                requests: vec![TargetRequest {
                    url: "https://api.example.com/data".parse()?,
                    method: Some(http::Method::POST),
                    headers: Some(
                        (&[(http::header::CONTENT_TYPE, "application/json".to_string())]
                            .into_iter()
                            .collect::<std::collections::HashMap<_, _>>())
                            .try_into()?,
                    ),
                    body: Some(json!({"key": "value"})),
                    media_type: Some("application/json".parse()?),
                    accept_statuses: Some(
                        [http::StatusCode::OK, http::StatusCode::CREATED]
                            .into_iter()
                            .collect(),
                    ),
                    accept_invalid_certificates: true,
                }],
                configurator: Some("(() => context)()".to_string()),
                extractor: Some("(() => ({ body: context.body }))()".to_string()),
                params: None,
            }),
            notifications: true,
        };
        let json = serde_json::to_value(&value)?;
        let target = &json["target"];

        assert_eq!(target["url"], "https://api.example.com/data");
        assert_eq!(target["method"], "POST");
        assert_eq!(target["headers"]["content-type"], "application/json");
        assert_eq!(target["body"], json!({"key": "value"}));
        assert_eq!(target["mediaType"], "application/json");
        assert_eq!(target["acceptInvalidCertificates"], true);
        assert_eq!(target["configurator"], "(() => context)()");
        assert_eq!(target["extractor"], "(() => ({ body: context.body }))()");

        let statuses = target["acceptStatuses"].as_array().unwrap();
        let mut status_values: Vec<u16> = statuses
            .iter()
            .map(|s| s.as_u64().unwrap() as u16)
            .collect();
        status_values.sort();
        assert_eq!(status_values, vec![200, 201]);

        assert!(target.get("type").is_none());
        assert!(target.get("requests").is_none());
        assert!(target.get("params").is_none());

        Ok(())
    }

    #[test]
    fn api_target_empty_requests_produces_empty_object() -> anyhow::Result<()> {
        let value = RetrackTrackerValue {
            id: uuid!("00000000-0000-0000-0000-000000000001"),
            enabled: true,
            config: minimal_config(),
            target: TrackerTarget::Api(ApiTarget {
                requests: vec![],
                configurator: None,
                extractor: None,
                params: None,
            }),
            notifications: false,
        };
        assert_json_snapshot!(serde_json::to_value(&value)?["target"], @"{}");
        Ok(())
    }
}
