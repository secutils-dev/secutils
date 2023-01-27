use crate::utils::{UtilsCertificatesAction, UtilsWebSecurityAction, UtilsWebhooksAction};
use serde_derive::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsAction {
    Certificates(UtilsCertificatesAction),
    Webhooks(UtilsWebhooksAction),
    WebSecurity(UtilsWebSecurityAction),
}
