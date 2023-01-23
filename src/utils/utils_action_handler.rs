use crate::{
    api::Api,
    users::User,
    utils::{
        UtilsAction, UtilsActionResult, UtilsCertificatesActionHandler,
        UtilsWebSecurityActionHandler,
    },
};

pub struct UtilsActionHandler;
impl UtilsActionHandler {
    pub async fn handle(
        user: User,
        api: &Api,
        action: UtilsAction,
    ) -> anyhow::Result<UtilsActionResult> {
        match action {
            UtilsAction::Certificates(action) => {
                UtilsCertificatesActionHandler::handle(user, api, action)
                    .await
                    .map(UtilsActionResult::Certificates)
            }
            UtilsAction::WebSecurity(action) => {
                UtilsWebSecurityActionHandler::handle(user, api, action)
                    .await
                    .map(UtilsActionResult::WebSecurity)
            }
        }
    }
}
