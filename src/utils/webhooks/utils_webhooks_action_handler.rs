use crate::{
    api::Api,
    users::User,
    utils::{UtilsWebhooksAction, UtilsWebhooksActionResult},
};

pub struct UtilsWebhooksActionHandler;
impl UtilsWebhooksActionHandler {
    pub async fn handle(
        user: User,
        api: &Api,
        action: UtilsWebhooksAction,
    ) -> anyhow::Result<UtilsWebhooksActionResult> {
        match action {
            UtilsWebhooksAction::GetAutoRespondersRequests {
                auto_responder_name,
            } => {
                let auto_responders_api = api.auto_responders();
                let requests = if let Some(auto_responder) = auto_responders_api
                    .get_auto_responder(user.id, &auto_responder_name)
                    .await?
                {
                    auto_responders_api
                        .get_requests(user.id, &auto_responder)
                        .await?
                } else {
                    Vec::with_capacity(0)
                };

                Ok(UtilsWebhooksActionResult::GetAutoRespondersRequests { requests })
            }
        }
    }
}
