use crate::{
    api::Api,
    users::User,
    utils::{UtilsCertificatesExecutor, UtilsRequest, UtilsResponse},
};

pub struct UtilsExecutor {}
impl UtilsExecutor {
    pub async fn execute(
        user: User,
        api: &Api,
        request: UtilsRequest,
    ) -> anyhow::Result<UtilsResponse> {
        match request {
            UtilsRequest::Certificates(request) => {
                UtilsCertificatesExecutor::execute(user, api, request)
                    .await
                    .map(UtilsResponse::Certificates)
            }
        }
    }
}
