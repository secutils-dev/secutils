use crate::utils::certificates::UtilsCertificatesResponse;

use serde_derive::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsResponse {
    Certificates(UtilsCertificatesResponse),
}
