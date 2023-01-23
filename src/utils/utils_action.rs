use crate::utils::{certificates::UtilsCertificatesAction, UtilsWebSecurityAction};
use serde_derive::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "value")]
pub enum UtilsAction {
    Certificates(UtilsCertificatesAction),
    WebSecurity(UtilsWebSecurityAction),
}
