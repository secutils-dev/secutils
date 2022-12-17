use crate::utils::{UtilsCertificatesExecutor, UtilsRequest, UtilsResponse};

pub struct UtilsExecutor {}
impl UtilsExecutor {
    pub async fn execute(request: UtilsRequest) -> anyhow::Result<UtilsResponse> {
        match request {
            UtilsRequest::Certificates(request) => UtilsCertificatesExecutor::execute(request)
                .await
                .map(UtilsResponse::Certificates),
        }
    }
}
