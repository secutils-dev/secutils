use serde::Serialize;
use std::{collections::HashMap, time::Duration};
use url::Url;

/// Scripts to inject into the web page before extracting resources to track.
#[derive(Serialize, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct WebScraperResourcesRequestScripts<'a> {
    /// Optional script used to `filter_map` resource that needs to be tracked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_filter_map: Option<&'a str>,
}

impl WebScraperResourcesRequestScripts<'_> {
    /// Returns `true` if none of the scripts are set.
    pub fn is_empty(&self) -> bool {
        self.resource_filter_map.is_none()
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

    /// Optional content of the web page that has been extracted previously.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<&'a HashMap<String, String>>,
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
            headers: None,
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

    /// Sets headers to attach to every request to the tracked web page.
    pub fn set_headers(self, headers: &'a HashMap<String, String>) -> Self {
        Self {
            headers: Some(headers),
            ..self
        }
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
                resource_filter_map: Some("return resource;")
            },
            headers: Some(
                &[("cookie".to_string(), "my-cookie".to_string())]
                    .into_iter()
                    .collect(),
            ),
        }, @r###"
        {
          "url": "http://localhost:1234/my/app?q=2",
          "timeout": 100,
          "delay": 200,
          "waitSelector": "body",
          "scripts": {
            "resourceFilterMap": "return resource;"
          },
          "headers": {
            "cookie": "my-cookie"
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
        assert!(request.headers.is_none());

        Ok(())
    }

    #[test]
    fn scripts_is_empty() -> anyhow::Result<()> {
        let scripts = WebScraperResourcesRequestScripts {
            resource_filter_map: None,
        };
        assert!(scripts.is_empty());

        let scripts = WebScraperResourcesRequestScripts {
            resource_filter_map: Some("return resource.url !== undefined;"),
        };
        assert!(!scripts.is_empty());

        Ok(())
    }
}
