mod certificates;
mod util;
mod utils_executor;
mod utils_request;
mod utils_response;
mod web_security;
mod webhooks;

pub use self::{
    certificates::{
        CertificateFormat, PublicKeyAlgorithm, SelfSignedCertificate, SignatureAlgorithm,
        UtilsCertificatesExecutor, UtilsCertificatesRequest, UtilsCertificatesResponse,
    },
    util::Util,
    utils_executor::UtilsExecutor,
    utils_request::UtilsRequest,
    utils_response::UtilsResponse,
    web_security::{
        ContentSecurityPolicy, ContentSecurityPolicyDirective,
        ContentSecurityPolicySandboxDirectiveValue, ContentSecurityPolicySource,
        ContentSecurityPolicyWebrtcDirectiveValue, UtilsWebSecurityExecutor,
        UtilsWebSecurityRequest, UtilsWebSecurityResponse,
    },
    webhooks::{AutoResponder, AutoResponderMethod},
};

#[cfg(test)]
pub mod tests {
    use crate::utils::{PublicKeyAlgorithm, SelfSignedCertificate, SignatureAlgorithm};
    use time::OffsetDateTime;

    pub struct MockSelfSignedCertificate(SelfSignedCertificate);
    impl MockSelfSignedCertificate {
        pub fn new<N: Into<String>>(
            name: N,
            public_key_algorithm: PublicKeyAlgorithm,
            signature_algorithm: SignatureAlgorithm,
            not_valid_before: OffsetDateTime,
            not_valid_after: OffsetDateTime,
            version: u8,
        ) -> Self {
            Self(SelfSignedCertificate {
                name: name.into(),
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

        pub fn build(self) -> SelfSignedCertificate {
            self.0
        }
    }
}
