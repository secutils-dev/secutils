use crate::utils::{UtilsCertificatesAction, UtilsWebSecurityAction, UtilsWebhooksAction};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsAction {
    Certificates(UtilsCertificatesAction),
    Webhooks(UtilsWebhooksAction),
    WebSecurity(UtilsWebSecurityAction),
}
