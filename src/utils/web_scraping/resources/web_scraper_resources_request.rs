use serde::Serialize;
use url::Url;

/// Represents request to scrap web page resources.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebScraperResourcesRequest<'a> {
    /// URL of the web page to scrap resources for.
    pub url: &'a Url,

    /// Number of milliseconds to wait until page enters "idle" state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<usize>,

    /// Number of milliseconds to wait after page enters "idle" state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<usize>,

    /// Optional CSS selector to wait for before extracting resources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_selector: Option<&'a str>,
}

impl<'a> WebScraperResourcesRequest<'a> {
    /// Creates request with only the URL of the web page to scrap resources for, the rest of the
    /// parameters are omitted.
    pub fn with_default_parameters(url: &'a Url) -> Self {
        Self {
            url,
            timeout: None,
            delay: None,
            wait_selector: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WebScraperResourcesRequest;
    use insta::assert_json_snapshot;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebScraperResourcesRequest {
            url: &Url::parse("http://localhost:1234/my/app?q=2")?,
            timeout: Some(100),
            delay: Some(200),
            wait_selector: Some("body")
        }, @r###"
        {
          "url": "http://localhost:1234/my/app?q=2",
          "timeout": 100,
          "delay": 200,
          "waitSelector": "body"
        }
        "###);

        Ok(())
    }

    #[test]
    fn serialization_with_default_parameters() -> anyhow::Result<()> {
        assert_json_snapshot!(WebScraperResourcesRequest::with_default_parameters(&Url::parse("http://localhost:1234/my/app?q=2")?), @r###"
        {
          "url": "http://localhost:1234/my/app?q=2"
        }
        "###);

        Ok(())
    }

    #[test]
    fn with_default_parameters() -> anyhow::Result<()> {
        let url = Url::parse("http://localhost:1234/my/app?q=2")?;
        let request = WebScraperResourcesRequest::with_default_parameters(&url);

        assert_eq!(request.url, &url);
        assert!(request.wait_selector.is_none());
        assert!(request.delay.is_none());
        assert!(request.timeout.is_none());

        Ok(())
    }
}
