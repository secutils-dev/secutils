use crate::utils::UtilsResource;

/// Describe custom util's resource operation.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum UtilsResourceOperation {
    CertificatesTemplateGenerate,
    CertificatesPrivateKeyExport,
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

impl TryFrom<(&UtilsResource, &str)> for UtilsResourceOperation {
    type Error = ();

    fn try_from((resource, operation): (&UtilsResource, &str)) -> Result<Self, Self::Error> {
        match resource {
            // Private keys custom actions.
            UtilsResource::CertificatesPrivateKeys if operation == "export" => {
                Ok(UtilsResourceOperation::CertificatesPrivateKeyExport)
            }

            // Certificate templates custom actions.
            UtilsResource::CertificatesTemplates if operation == "generate" => {
                Ok(UtilsResourceOperation::CertificatesTemplateGenerate)
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

    #[test]
    fn properly_checks_if_action_requires_params() {
        assert!(UtilsResourceOperation::CertificatesPrivateKeyExport.requires_params());

        assert!(UtilsResourceOperation::CertificatesTemplateGenerate.requires_params());

        assert!(UtilsResourceOperation::WebScrapingGetHistory.requires_params());
        assert!(!UtilsResourceOperation::WebScrapingClearHistory.requires_params());

        assert!(
            UtilsResourceOperation::WebSecurityContentSecurityPolicySerialize.requires_params()
        );
    }

    #[test]
    fn properly_parses_resource_action_operation() {
        assert_eq!(
            UtilsResourceOperation::try_from((&UtilsResource::CertificatesPrivateKeys, "export")),
            Ok(UtilsResourceOperation::CertificatesPrivateKeyExport)
        );
        assert!(UtilsResourceOperation::try_from((
            &UtilsResource::CertificatesTemplates,
            "export"
        ))
        .is_err());

        assert_eq!(
            UtilsResourceOperation::try_from((&UtilsResource::CertificatesTemplates, "generate")),
            Ok(UtilsResourceOperation::CertificatesTemplateGenerate)
        );
        assert!(UtilsResourceOperation::try_from((
            &UtilsResource::CertificatesPrivateKeys,
            "generate"
        ))
        .is_err());

        assert!(UtilsResourceOperation::try_from((
            &UtilsResource::CertificatesPrivateKeys,
            "share"
        ))
        .is_err());

        assert!(UtilsResourceOperation::try_from((
            &UtilsResource::CertificatesPrivateKeys,
            "unshare"
        ))
        .is_err());

        assert_eq!(
            UtilsResourceOperation::try_from((&UtilsResource::WebScrapingResources, "history")),
            Ok(UtilsResourceOperation::WebScrapingGetHistory)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((&UtilsResource::WebScrapingResources, "clear")),
            Ok(UtilsResourceOperation::WebScrapingClearHistory)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((&UtilsResource::WebScrapingContent, "history")),
            Ok(UtilsResourceOperation::WebScrapingGetHistory)
        );
        assert_eq!(
            UtilsResourceOperation::try_from((&UtilsResource::WebScrapingContent, "clear")),
            Ok(UtilsResourceOperation::WebScrapingClearHistory)
        );
        assert!(UtilsResourceOperation::try_from((
            &UtilsResource::CertificatesPrivateKeys,
            "history"
        ))
        .is_err());

        assert_eq!(
            UtilsResourceOperation::try_from((
                &UtilsResource::WebSecurityContentSecurityPolicies,
                "serialize"
            )),
            Ok(UtilsResourceOperation::WebSecurityContentSecurityPolicySerialize)
        );
        assert!(UtilsResourceOperation::try_from((
            &UtilsResource::WebSecurityContentSecurityPolicies,
            "generate"
        ))
        .is_err());
    }
}
