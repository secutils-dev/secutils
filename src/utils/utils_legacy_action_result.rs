use crate::utils::{
    UtilsWebScrapingActionResult, UtilsWebSecurityActionResult, UtilsWebhooksActionResult,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsLegacyActionResult {
    Webhooks(UtilsWebhooksActionResult),
    WebScraping(UtilsWebScrapingActionResult),
    WebSecurity(UtilsWebSecurityActionResult),
}
