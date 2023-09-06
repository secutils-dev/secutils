use serde::Serialize;
use std::time::Duration;
use url::Url;

/// Scripts to inject into the web page before extracting resources to track.
#[derive(Serialize, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct WebScraperResourcesRequestScripts<'a> {
    /// Optional script used to filter resource that need to be tracked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_filter: Option<&'a str>,
}

impl<'a> WebScraperResourcesRequestScripts<'a> {
    /// Returns `true` if none of the scripts are set.
    pub fn is_empty(&self) -> bool {
        self.resource_filter.is_none()
    }
}

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
    pub delay: Option<u128>,

    /// Optional CSS selector to wait for before extracting resources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_selector: Option<&'a str>,

    /// Optional scripts to inject into the web page before extracting resources to track..
    #[serde(skip_serializing_if = "WebScraperResourcesRequestScripts::is_empty")]
    pub scripts: WebScraperResourcesRequestScripts<'a>,
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
            scripts: Default::default(),
        }
    }

    /// Sets the delay to wait after web page enters "idle" state to start tracking resources.
    pub fn set_delay(self, delay: Duration) -> Self {
        Self {
            delay: Some(delay.as_millis()),
            ..self
        }
    }

    /// Sets scripts to inject into the web page before extracting resources to track.
    pub fn set_scripts(self, scripts: WebScraperResourcesRequestScripts<'a>) -> Self {
        Self { scripts, ..self }
    }
}

#[cfg(test)]
mod tests {
    use super::{WebScraperResourcesRequest, WebScraperResourcesRequestScripts};
    use insta::assert_json_snapshot;
    use url::Url;

    #[test]
    fn serialization() -> anyhow::Result<()> {
        assert_json_snapshot!(WebScraperResourcesRequest {
            url: &Url::parse("http://localhost:1234/my/app?q=2")?,
            timeout: Some(100),
            delay: Some(200),
            wait_selector: Some("body"),
            scripts: WebScraperResourcesRequestScripts {
                resource_filter: Some("return resource.url !== undefined;")
            }
        }, @r###"
        {
          "url": "http://localhost:1234/my/app?q=2",
          "timeout": 100,
          "delay": 200,
          "waitSelector": "body",
          "scripts": {
            "resourceFilter": "return resource.url !== undefined;"
          }
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

        assert_json_snapshot!(WebScraperResourcesRequest::with_default_parameters(&Url::parse("http://localhost:1234/my/app?q=2")?).set_scripts(Default::default()), @r###"
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
        assert!(request.scripts.is_empty());

        Ok(())
    }

    #[test]
    fn scripts_is_empty() -> anyhow::Result<()> {
        let scripts = WebScraperResourcesRequestScripts {
            resource_filter: None,
        };
        assert!(scripts.is_empty());

        let scripts = WebScraperResourcesRequestScripts {
            resource_filter: Some("return resource.url !== undefined;"),
        };
        assert!(!scripts.is_empty());

        Ok(())
    }
}
