use crate::utils::{
    UtilsCertificatesActionResult, UtilsWebSecurityActionResult, UtilsWebhooksActionResult,
};
use serde_derive::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsActionResult {
    Certificates(UtilsCertificatesActionResult),
    Webhooks(UtilsWebhooksActionResult),
    WebSecurity(UtilsWebSecurityActionResult),
}
