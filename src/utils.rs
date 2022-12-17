mod certificates;
mod util;
mod utils_executor;
mod utils_request;
mod utils_response;
mod webhooks;

pub use self::{
    certificates::{
        PublicKeyAlgorithm, SignatureAlgorithm, UtilsCertificatesExecutor,
        UtilsCertificatesRequest, UtilsCertificatesResponse,
    },
    util::Util,
    utils_executor::UtilsExecutor,
    utils_request::UtilsRequest,
    utils_response::UtilsResponse,
    webhooks::{AutoResponder, AutoResponderMethod, USER_PROFILE_DATA_KEY_AUTO_RESPONDERS},
};

#[cfg(test)]
pub mod tests {
    use crate::utils::{AutoResponder, AutoResponderMethod};

    pub struct MockAutoResponder {
        auto_responder: AutoResponder,
    }

    impl MockAutoResponder {
        pub fn new<A: Into<String>>(
            alias: A,
            method: AutoResponderMethod,
            status_code: u16,
        ) -> Self {
            Self {
                auto_responder: AutoResponder {
                    alias: alias.into(),
                    method,
                    status_code,
                    headers: None,
                    body: None,
                },
            }
        }

        pub fn set_headers(mut self, headers: Vec<(String, String)>) -> Self {
            self.auto_responder.headers = Some(headers);
            self
        }

        pub fn set_body<B: Into<String>>(mut self, body: B) -> Self {
            self.auto_responder.body = Some(body.into());
            self
        }

        pub fn build(self) -> AutoResponder {
            self.auto_responder
        }
    }
}
