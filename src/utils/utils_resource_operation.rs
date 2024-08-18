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
    WebScrapingGetHistory,
    WebScrapingClearHistory,
    WebSecurityContentSecurityPolicySerialize,
}

impl UtilsResourceOperation {
    /// Returns true if the operation requires parameters (via HTTP body).
    pub fn requires_params(&self) -> bool {
        matches!(
            self,
            Self::CertificatesTemplateGenerate
                | Self::CertificatesPrivateKeyExport
                | Self::WebScrapingGetHistory
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
            UtilsResource::WebScrapingResources | UtilsResource::WebScrapingContent
                if operation == "history" =>
            {
                Ok(UtilsResourceOperation::WebScrapingGetHistory)
            }
            UtilsResource::WebScrapingResources | UtilsResource::WebScrapingContent
                if operation == "clear" =>
            {
                Ok(UtilsResourceOperation::WebScrapingClearHistory)
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

        assert!(UtilsResourceOperation::WebScrapingGetHistory.requires_params());
        assert!(!UtilsResourceOperation::WebScrapingClearHistory.requires_params());

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
        assert!(UtilsResourceOperation::try_from((
            &UtilsResource::CertificatesTemplates,
            "export",
            &Method::POST
        ))
        .is_err());

        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::CertificatesTemplates,
                "generate",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::CertificatesTemplateGenerate)
        );
        assert!(UtilsResourceOperation::try_from((
            &UtilsResource::CertificatesPrivateKeys,
            "generate",
            &Method::POST
        ))
        .is_err());

        assert!(UtilsResourceOperation::try_from((
            &UtilsResource::CertificatesPrivateKeys,
            "share",
            &Method::POST
        ))
        .is_err());

        assert!(UtilsResourceOperation::try_from((
            &UtilsResource::CertificatesPrivateKeys,
            "unshare",
            &Method::POST
        ))
        .is_err());

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
                &UtilsResource::WebScrapingResources,
                "history",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingGetHistory)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingResources,
                "clear",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingClearHistory)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingContent,
                "history",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingGetHistory)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebScrapingContent,
                "clear",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebScrapingClearHistory)
        );
        assert!(UtilsResourceOperation::try_from((
            &UtilsResource::CertificatesPrivateKeys,
            "history",
            &Method::POST
        ))
        .is_err());

        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebSecurityContentSecurityPolicies,
                "serialize",
                &Method::POST
            )),
            Ok(UtilsResourceOperation::WebSecurityContentSecurityPolicySerialize)
        );
        assert!(UtilsResourceOperation::try_from((
            &UtilsResource::WebSecurityContentSecurityPolicies,
            "generate",
            &Method::POST
        ))
        .is_err());
    }
}
