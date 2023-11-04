use crate::utils::{UtilsWebSecurityActionResult, UtilsWebhooksActionResult};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsLegacyActionResult {
    Webhooks(UtilsWebhooksActionResult),
    WebSecurity(UtilsWebSecurityActionResult),
}
