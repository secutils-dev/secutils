use crate::utils::{certificates::UtilsCertificatesRequest, UtilsWebSecurityRequest};
use serde_derive::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsRequest {
    Certificates(UtilsCertificatesRequest),
    WebSecurity(UtilsWebSecurityRequest),
}
