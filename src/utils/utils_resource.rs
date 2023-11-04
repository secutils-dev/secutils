#[derive(Debug, Copy, Clone, PartialEq)]
pub enum UtilsResource {
    CertificatesTemplates,
    CertificatesPrivateKeys,
    WebScrapingResources,
}

impl TryFrom<(&str, &str)> for UtilsResource {
    type Error = ();

    fn try_from((area, resource): (&str, &str)) -> Result<Self, Self::Error> {
        match (area, resource) {
            ("certificates", "templates") => Ok(UtilsResource::CertificatesTemplates),
            ("certificates", "private_keys") => Ok(UtilsResource::CertificatesPrivateKeys),
            ("web_scraping", "resources") => Ok(UtilsResource::WebScrapingResources),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UtilsResource;

    #[test]
    fn properly_parses_resource() {
        assert_eq!(
            UtilsResource::try_from(("certificates", "templates")),
            Ok(UtilsResource::CertificatesTemplates)
        );
        assert_eq!(
            UtilsResource::try_from(("certificates", "private_keys")),
            Ok(UtilsResource::CertificatesPrivateKeys)
        );
        assert_eq!(
            UtilsResource::try_from(("web_scraping", "resources")),
            Ok(UtilsResource::WebScrapingResources)
        );

        assert!(UtilsResource::try_from(("certificates_", "templates")).is_err());
        assert!(UtilsResource::try_from(("certificates_", "private_keys")).is_err());
        assert!(UtilsResource::try_from(("web_scraping", "_resources")).is_err());
    }
}
