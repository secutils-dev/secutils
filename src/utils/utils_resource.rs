#[derive(Debug, Copy, Clone, PartialEq)]
pub enum UtilsResource {
    CertificatesTemplates,
    CertificatesPrivateKeys,
    WebhooksResponders,
    WebScrapingResources,
    WebScrapingContent,
    WebSecurityContentSecurityPolicies,
}

impl TryFrom<(&str, &str)> for UtilsResource {
    type Error = ();

    fn try_from((area, resource): (&str, &str)) -> Result<Self, Self::Error> {
        match (area, resource) {
            ("certificates", "templates") => Ok(UtilsResource::CertificatesTemplates),
            ("certificates", "private_keys") => Ok(UtilsResource::CertificatesPrivateKeys),
            ("webhooks", "responders") => Ok(UtilsResource::WebhooksResponders),
            ("web_scraping", "resources") => Ok(UtilsResource::WebScrapingResources),
            ("web_scraping", "content") => Ok(UtilsResource::WebScrapingContent),
            ("web_security", "csp") => Ok(UtilsResource::WebSecurityContentSecurityPolicies),
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
            UtilsResource::try_from(("webhooks", "responders")),
            Ok(UtilsResource::WebhooksResponders)
        );
        assert_eq!(
            UtilsResource::try_from(("web_scraping", "resources")),
            Ok(UtilsResource::WebScrapingResources)
        );
        assert_eq!(
            UtilsResource::try_from(("web_scraping", "content")),
            Ok(UtilsResource::WebScrapingContent)
        );
        assert_eq!(
            UtilsResource::try_from(("web_security", "csp")),
            Ok(UtilsResource::WebSecurityContentSecurityPolicies)
        );

        assert!(UtilsResource::try_from(("certificates_", "templates")).is_err());
        assert!(UtilsResource::try_from(("certificates_", "private_keys")).is_err());
        assert!(UtilsResource::try_from(("webhooks", "_responders")).is_err());
        assert!(UtilsResource::try_from(("web_scraping", "_resources")).is_err());
        assert!(UtilsResource::try_from(("web_scraping", "_content")).is_err());
        assert!(UtilsResource::try_from(("web_security", "_csp")).is_err());
    }
}
