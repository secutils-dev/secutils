pub mod api_ext;
mod certificates;
mod database_ext;
mod user_share_ext;
mod util;
mod utils_action;
mod utils_action_result;
mod utils_action_validation;
mod web_scraping;
mod web_security;
mod webhooks;

pub use self::{
    certificates::{
        CertificatesApi, ExportFormat, ExtendedKeyUsage, KeyUsage, PrivateKey, PrivateKeyAlgorithm,
        PrivateKeyEllipticCurve, PrivateKeySize, SelfSignedCertificate, SignatureAlgorithm,
        UtilsCertificatesAction, UtilsCertificatesActionResult, Version,
    },
    util::Util,
    utils_action::UtilsAction,
    utils_action_result::UtilsActionResult,
    web_scraping::{
        UtilsWebScrapingAction, UtilsWebScrapingActionResult, WebPageResource,
        WebPageResourceContent, WebPageResourceContentData, WebPageResourceDiffStatus,
        WebPageResourcesRevision, WebPageResourcesTracker, WebPageResourcesTrackerScripts,
        WebScraperResource, WebScraperResourcesRequest, WebScraperResourcesRequestScripts,
        WebScraperResourcesResponse,
    },
    web_security::{
        ContentSecurityPolicy, ContentSecurityPolicyDirective, ContentSecurityPolicyImportType,
        ContentSecurityPolicyRequireTrustedTypesForDirectiveValue,
        ContentSecurityPolicySandboxDirectiveValue, ContentSecurityPolicySource,
        ContentSecurityPolicyTrustedTypesDirectiveValue, ContentSecurityPolicyWebrtcDirectiveValue,
        UtilsWebSecurityAction, UtilsWebSecurityActionResult,
    },
    webhooks::{
        AutoResponder, AutoResponderMethod, AutoResponderRequest, UtilsWebhooksAction,
        UtilsWebhooksActionResult,
    },
};

#[cfg(test)]
pub mod tests {
    use crate::utils::{
        ExtendedKeyUsage, KeyUsage, PrivateKeyAlgorithm, SelfSignedCertificate, SignatureAlgorithm,
        Version,
    };
    use time::OffsetDateTime;

    pub use super::web_scraping::tests::MockWebPageResourcesTrackerBuilder;

    pub struct MockSelfSignedCertificate(SelfSignedCertificate);
    impl MockSelfSignedCertificate {
        pub fn new<N: Into<String>>(
            name: N,
            public_key_algorithm: PrivateKeyAlgorithm,
            signature_algorithm: SignatureAlgorithm,
            not_valid_before: OffsetDateTime,
            not_valid_after: OffsetDateTime,
            version: Version,
        ) -> Self {
            Self(SelfSignedCertificate {
                name: name.into(),
                common_name: None,
                country: None,
                state_or_province: None,
                locality: None,
                organization: None,
                organizational_unit: None,
                key_algorithm: public_key_algorithm,
                signature_algorithm,
                not_valid_before,
                not_valid_after,
                version,
                is_ca: false,
                key_usage: None,
                extended_key_usage: None,
            })
        }

        pub fn set_is_ca(mut self) -> Self {
            self.0.is_ca = true;
            self
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

        pub fn add_key_usage(mut self, key_usage: KeyUsage) -> Self {
            if let Some(key_usage_list) = self.0.key_usage.as_mut() {
                key_usage_list.insert(key_usage);
            } else {
                self.0.key_usage = Some([key_usage].into_iter().collect());
            }
            self
        }

        pub fn add_extended_key_usage(mut self, key_usage: ExtendedKeyUsage) -> Self {
            if let Some(key_usage_list) = self.0.extended_key_usage.as_mut() {
                key_usage_list.insert(key_usage);
            } else {
                self.0.extended_key_usage = Some([key_usage].into_iter().collect());
            }
            self
        }

        pub fn build(self) -> SelfSignedCertificate {
            self.0
        }
    }
}
