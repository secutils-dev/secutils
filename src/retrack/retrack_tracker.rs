use crate::retrack::tags::{RETRACK_NOTIFICATIONS_TAG, get_tag_value};
use retrack_types::trackers::{Tracker, TrackerConfig, TrackerTarget};
use serde_derive::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(untagged, rename_all = "camelCase")]
pub enum RetrackTracker {
    Reference { id: Uuid },
    Value(Box<RetrackTrackerValue>),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RetrackTrackerValue {
    pub id: Uuid,
    pub enabled: bool,
    pub config: TrackerConfig,
    pub target: TrackerTarget,
    pub notifications: bool,
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
