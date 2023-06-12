use crate::utils::{
    UtilsCertificatesActionResult, UtilsWebScrappingActionResult, UtilsWebSecurityActionResult,
    UtilsWebhooksActionResult,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsActionResult {
    Certificates(UtilsCertificatesActionResult),
    Webhooks(UtilsWebhooksActionResult),
    WebScrapping(UtilsWebScrappingActionResult),
    WebSecurity(UtilsWebSecurityActionResult),
}
