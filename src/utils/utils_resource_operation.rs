use crate::utils::UtilsResource;
use actix_web::http::Method;

/// Describe custom util's resource operation.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum UtilsResourceOperation {
    CertificatesTemplateGenerate,
    CertificatesPrivateKeyExport,
    WebhooksRespondersGetHistory,
    WebhooksRespondersClearHistory,
    WebhooksRespondersGetStats,
    WebScrapingPageGetHistory,
    WebScrapingPageClearHistory,
    WebSecurityContentSecurityPolicySerialize,
}

impl UtilsResourceOperation {
    /// Returns true if the operation requires parameters (via HTTP body).
    pub fn requires_params(&self) -> bool {
        matches!(
            self,
            Self::CertificatesTemplateGenerate
                | Self::CertificatesPrivateKeyExport
                | Self::WebScrapingPageGetHistory
                | Self::WebSecurityContentSecurityPolicySerialize
        )
    }
}

impl TryFrom<(&UtilsResource, &str, &Method)> for UtilsResourceOperation {
    type Error = ();

    fn try_from(
        (resource, operation, method): (&UtilsResource, &str, &Method),
    ) -> Result<Self, Self::Error> {
        match resource {
            // Private keys custom actions.
            UtilsResource::CertificatesPrivateKeys if operation == "export" => {
                Ok(UtilsResourceOperation::CertificatesPrivateKeyExport)
            }

            // Certificate templates custom actions.
            UtilsResource::CertificatesTemplates if operation == "generate" => {
                Ok(UtilsResourceOperation::CertificatesTemplateGenerate)
            }

            // Webhooks custom actions.
            UtilsResource::WebhooksResponders if operation == "history" => {
                Ok(UtilsResourceOperation::WebhooksRespondersGetHistory)
            }
            UtilsResource::WebhooksResponders if operation == "clear" => {
                Ok(UtilsResourceOperation::WebhooksRespondersClearHistory)
            }
            UtilsResource::WebhooksResponders if operation == "stats" && method == Method::GET => {
                Ok(UtilsResourceOperation::WebhooksRespondersGetStats)
            }

            // Web scraping custom actions.
            UtilsResource::WebScrapingPage if operation == "history" => {
                Ok(UtilsResourceOperation::WebScrapingPageGetHistory)
            }
            UtilsResource::WebScrapingPage if operation == "clear" => {
                Ok(UtilsResourceOperation::WebScrapingPageClearHistory)
            }

            // Web security custom actions.
            UtilsResource::WebSecurityContentSecurityPolicies if operation == "serialize" => {
                Ok(UtilsResourceOperation::WebSecurityContentSecurityPolicySerialize)
            }

            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UtilsResourceOperation;
    use crate::utils::UtilsResource;
    use actix_web::http::Method;

    #[test]
    fn properly_checks_if_action_requires_params() {
        assert!(UtilsResourceOperation::CertificatesPrivateKeyExport.requires_params());

        assert!(UtilsResourceOperation::CertificatesTemplateGenerate.requires_params());

        assert!(!UtilsResourceOperation::WebhooksRespondersGetHistory.requires_params());
        assert!(!UtilsResourceOperation::WebhooksRespondersClearHistory.requires_params());
        assert!(!UtilsResourceOperation::WebhooksRespondersGetStats.requires_params());

        assert!(UtilsResourceOperation::WebScrapingPageGetHistory.requires_params());
        assert!(!UtilsResourceOperation::WebScrapingPageClearHistory.requires_params());

        assert!(
            UtilsResourceOperation::WebSecurityContentSecurityPolicySerialize.requires_params()
        );
    }

    #[test]
    fn properly_parses_resource_action_operation() {
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::CertificatesPrivateKeys,
                "export",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::CertificatesPrivateKeyExport)
        );
        assert!(
            UtilsResourceOperation::try_from((
                &UtilsResource::CertificatesTemplates,
                "export",
                &Method::POST
            ))
            .is_err()
        );

        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::CertificatesTemplates,
                "generate",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::CertificatesTemplateGenerate)
        );
        assert!(
            UtilsResourceOperation::try_from((
                &UtilsResource::CertificatesPrivateKeys,
                "generate",
                &Method::POST
            ))
            .is_err()
        );

        assert!(
            UtilsResourceOperation::try_from((
                &UtilsResource::CertificatesPrivateKeys,
                "share",
                &Method::POST
            ))
            .is_err()
        );

        assert!(
            UtilsResourceOperation::try_from((
                &UtilsResource::CertificatesPrivateKeys,
                "unshare",
                &Method::POST
            ))
            .is_err()
        );

        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebhooksResponders,
                "history",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebhooksRespondersGetHistory)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebhooksResponders,
                "clear",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebhooksRespondersClearHistory)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebhooksResponders,
                "stats",
                &Method::GET
            )),
            Ok(UtilsResourceOperation::WebhooksRespondersGetStats)
        );

        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingPage,
                "history",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingPageGetHistory)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingPage,
                "clear",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingPageClearHistory)
        );
        assert!(
            UtilsResourceOperation::try_from((
                &UtilsResource::CertificatesPrivateKeys,
                "history",
                &Method::POST
            ))
            .is_err()
        );

        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebSecurityContentSecurityPolicies,
                "serialize",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebSecurityContentSecurityPolicySerialize)
        );
        assert!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebSecurityContentSecurityPolicies,
                "generate",
                &Method::POST
            ))
            .is_err()
        );
    }
}
