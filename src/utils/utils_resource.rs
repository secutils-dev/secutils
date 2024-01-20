#[derive(Debug, Copy, Clone, PartialEq)]
pub enum UtilsResource {
    CertificatesTemplates,
    CertificatesPrivateKeys,
    WebhooksResponders,
    WebScrapingResources,
    WebScrapingContent,
    WebSecurityContentSecurityPolicies,
}

impl From<UtilsResource> for (&str, &str) {
    fn from(value: UtilsResource) -> Self {
        match value {
            UtilsResource::CertificatesTemplates => ("certificates", "templates"),
            UtilsResource::CertificatesPrivateKeys => ("certificates", "private_keys"),
            UtilsResource::WebhooksResponders => ("webhooks", "responders"),
            UtilsResource::WebScrapingResources => ("web_scraping", "resources"),
            UtilsResource::WebScrapingContent => ("web_scraping", "content"),
            UtilsResource::WebSecurityContentSecurityPolicies => ("web_security", "csp"),
        }
    }
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

    #[test]
    fn correctly_converts_into_resource_tuple() {
        type ResourceTuple = (&'static str, &'static str);
        assert_eq!(
            ResourceTuple::from(UtilsResource::CertificatesTemplates),
            ("certificates", "templates")
        );
        assert_eq!(
            ResourceTuple::from(UtilsResource::CertificatesPrivateKeys),
            ("certificates", "private_keys")
        );
        assert_eq!(
            ResourceTuple::from(UtilsResource::WebhooksResponders),
            ("webhooks", "responders")
        );
        assert_eq!(
            ResourceTuple::from(UtilsResource::WebScrapingResources),
            ("web_scraping", "resources")
        );
        assert_eq!(
            ResourceTuple::from(UtilsResource::WebScrapingContent),
            ("web_scraping", "content")
        );
        assert_eq!(
            ResourceTuple::from(UtilsResource::WebSecurityContentSecurityPolicies),
            ("web_security", "csp")
        );
    }
}
