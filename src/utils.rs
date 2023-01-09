mod certificates;
mod util;
mod utils_executor;
mod utils_request;
mod utils_response;
mod webhooks;

pub use self::{
    certificates::{
        PublicKeyAlgorithm, RootCertificate, SignatureAlgorithm, UtilsCertificatesExecutor,
        UtilsCertificatesRequest, UtilsCertificatesResponse,
    },
    util::Util,
    utils_executor::UtilsExecutor,
    utils_request::UtilsRequest,
    utils_response::UtilsResponse,
    webhooks::{AutoResponder, AutoResponderMethod},
};

#[cfg(test)]
pub mod tests {
    use crate::utils::{
        AutoResponder, AutoResponderMethod, PublicKeyAlgorithm, RootCertificate, SignatureAlgorithm,
    };
    use time::OffsetDateTime;

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
                    requests_to_track: None,
                    status_code,
                    headers: None,
                    body: None,
                    delay: None,
                },
            }
        }

        pub fn set_requests_to_track(mut self, requests_to_track: usize) -> Self {
            self.auto_responder.requests_to_track = Some(requests_to_track);
            self
        }

        pub fn set_headers(mut self, headers: Vec<(String, String)>) -> Self {
            self.auto_responder.headers = Some(headers);
            self
        }

        pub fn set_body<B: Into<String>>(mut self, body: B) -> Self {
            self.auto_responder.body = Some(body.into());
            self
        }

        pub fn set_delay(mut self, delay: usize) -> Self {
            self.auto_responder.delay = Some(delay);
            self
        }

        pub fn build(self) -> AutoResponder {
            self.auto_responder
        }
    }

    pub struct MockRootCertificate(RootCertificate);
    impl MockRootCertificate {
        pub fn new<A: Into<String>>(
            alias: A,
            public_key_algorithm: PublicKeyAlgorithm,
            signature_algorithm: SignatureAlgorithm,
            not_valid_before: OffsetDateTime,
            not_valid_after: OffsetDateTime,
            version: u8,
        ) -> Self {
            Self(RootCertificate {
                alias: alias.into(),
                common_name: None,
                country: None,
                state_or_province: None,
                locality: None,
                organization: None,
                organizational_unit: None,
                public_key_algorithm,
                signature_algorithm,
                not_valid_before,
                not_valid_after,
                version,
            })
        }

        pub fn set_common_name<CN: Into<String>>(mut self, value: CN) -> Self {
            self.0.common_name = Some(value.into());
            self
        }

        pub fn set_country<C: Into<String>>(mut self, value: C) -> Self {
            self.0.country = Some(value.into());
            self
        }

        pub fn set_state_or_province<S: Into<String>>(mut self, value: S) -> Self {
            self.0.state_or_province = Some(value.into());
            self
        }

        pub fn set_locality<L: Into<String>>(mut self, value: L) -> Self {
            self.0.locality = Some(value.into());
            self
        }

        pub fn set_organization<L: Into<String>>(mut self, value: L) -> Self {
            self.0.organization = Some(value.into());
            self
        }

        pub fn set_organization_unit<L: Into<String>>(mut self, value: L) -> Self {
            self.0.organizational_unit = Some(value.into());
            self
        }

        pub fn build(self) -> RootCertificate {
            self.0
        }
    }
}
