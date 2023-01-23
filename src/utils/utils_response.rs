use crate::utils::{UtilsCertificatesResponse, UtilsWebSecurityResponse};
use serde_derive::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsResponse {
    Certificates(UtilsCertificatesResponse),
    WebSecurity(UtilsWebSecurityResponse),
}
