use crate::utils::UtilsResource;

/// Describe custom util's resource operation.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum UtilsResourceOperation {
    CertificatesTemplateGenerate,
    CertificatesTemplateShare,
    CertificatesTemplateUnshare,
    CertificatesPrivateKeyExport,
}

impl UtilsResourceOperation {
    /// Returns true if the operation requires parameters (via HTTP body).
    pub fn requires_params(&self) -> bool {
        matches!(
            self,
            Self::CertificatesTemplateGenerate | Self::CertificatesPrivateKeyExport
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
            UtilsResource::CertificatesTemplates if operation == "share" => {
                Ok(UtilsResourceOperation::CertificatesTemplateShare)
            }
            UtilsResource::CertificatesTemplates if operation == "unshare" => {
                Ok(UtilsResourceOperation::CertificatesTemplateUnshare)
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
        assert!(!UtilsResourceOperation::CertificatesTemplateShare.requires_params());
        assert!(!UtilsResourceOperation::CertificatesTemplateUnshare.requires_params());
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

        assert_eq!(
            UtilsResourceOperation::try_from((&UtilsResource::CertificatesTemplates, "share")),
            Ok(UtilsResourceOperation::CertificatesTemplateShare)
        );
        assert!(UtilsResourceOperation::try_from((
            &UtilsResource::CertificatesPrivateKeys,
            "share"
        ))
        .is_err());

        assert_eq!(
            UtilsResourceOperation::try_from((&UtilsResource::CertificatesTemplates, "unshare")),
            Ok(UtilsResourceOperation::CertificatesTemplateUnshare)
        );
        assert!(UtilsResourceOperation::try_from((
            &UtilsResource::CertificatesPrivateKeys,
            "unshare"
        ))
        .is_err());
    }
}
