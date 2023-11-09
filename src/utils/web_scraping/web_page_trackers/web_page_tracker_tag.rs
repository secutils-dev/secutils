use crate::utils::WebPageTrackerKind;
use serde::{Deserialize, Serialize};

/// Trait that defines kind and utility types of a web page tracker of the specific type.
pub trait WebPageTrackerTag {
    const KIND: WebPageTrackerKind;
    type TrackerMeta: Clone + Serialize + for<'de> Deserialize<'de>;
    type TrackerData: Clone + Serialize + for<'de> Deserialize<'de>;
}
