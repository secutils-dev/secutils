use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusLevel {
    Available,
    Unavailable,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Status {
    pub version: String,
    pub level: StatusLevel,
}
