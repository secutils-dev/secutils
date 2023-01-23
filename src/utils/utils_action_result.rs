use crate::utils::{UtilsCertificatesActionResult, UtilsWebSecurityActionResult};
use serde_derive::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsActionResult {
    Certificates(UtilsCertificatesActionResult),
    WebSecurity(UtilsWebSecurityActionResult),
}
