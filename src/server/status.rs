use serde_derive::{Deserialize, Serialize};

#[derive(Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatusLevel {
    Available,
    Unavailable,
}

#[derive(Copy, Clone, Serialize, Deserialize)]
pub struct Status {
    pub level: StatusLevel,
}
