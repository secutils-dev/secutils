use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use futures::future::BoxFuture;
use reqwest::redirect::Policy as RedirectPolicy;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, OnceLock},
};
use url::Url;

static PROXY_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Trait for validating whether a URL is publicly accessible (not pointing to
/// private/internal networks). Implemented by `Network<DR, ET>` so we can
/// erase the generic parameters for storage in [`ProxyState`].
pub trait PublicUrlValidator: Send + Sync {
    fn is_public_web_url<'a>(&'a self, url: &'a Url) -> BoxFuture<'a, bool>;
}

/// State injected into Deno's `OpState` before script execution.
#[derive(Clone)]
pub struct ProxyState {
    pub url_validator: Arc<dyn PublicUrlValidator>,
    pub restrict_to_public_urls: bool,
    pub max_response_size: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyRequest {
    pub url: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>,
}

fn default_method() -> String {
    "GET".to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    #[serde(with = "serde_bytes")]
    pub body: Vec<u8>,
}

#[op2]
#[serde]
pub async fn op_proxy_request(
    state: Rc<RefCell<OpState>>,
    #[serde] request: ProxyRequest,
) -> Result<ProxyResponse, JsErrorBox> {
    let proxy = {
        let state = state.borrow();
        state.borrow::<ProxyState>().clone()
    };

    let url = Url::parse(&request.url)
        .map_err(|e| JsErrorBox::generic(format!("Invalid URL '{}': {e}", request.url)))?;

    if proxy.restrict_to_public_urls && !proxy.url_validator.is_public_web_url(&url).await {
        return Err(JsErrorBox::generic(format!(
            "URL not allowed (non-public address): {url}"
        )));
    }

    let method: http::Method = request
        .method
        .parse()
        .map_err(|_| JsErrorBox::generic(format!("Invalid HTTP method: '{}'", request.method)))?;

    let client = PROXY_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .redirect(RedirectPolicy::none())
            .build()
            .expect("Failed to build proxy HTTP client")
    });
    let mut req_builder = client.request(method, url);
    for (k, v) in &request.headers {
        let name = http::HeaderName::from_bytes(k.as_bytes())
            .map_err(|_| JsErrorBox::generic(format!("Invalid header name: '{k}'")))?;
        let value = http::HeaderValue::from_str(v)
            .map_err(|_| JsErrorBox::generic(format!("Invalid header value for '{k}'")))?;
        req_builder = req_builder.header(name, value);
    }
    if let Some(body) = request.body {
        req_builder = req_builder.body(body);
    }

    let response = req_builder.send().await.map_err(|e| {
        JsErrorBox::generic(if e.is_timeout() {
            format!("Upstream request timed out: {e}")
        } else if e.is_connect() {
            format!("Failed to connect to upstream: {e}")
        } else {
            format!("Upstream request failed: {e}")
        })
    })?;

    let status_code = response.status().as_u16();
    let headers: HashMap<String, String> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let body = response
        .bytes()
        .await
        .map_err(|e| JsErrorBox::generic(format!("Failed to read upstream response body: {e}")))?;

    if body.len() > proxy.max_response_size {
        return Err(JsErrorBox::generic(format!(
            "Upstream response body too large: {} bytes exceeds limit of {} bytes",
            body.len(),
            proxy.max_response_size
        )));
    }

    Ok(ProxyResponse {
        status_code,
        headers,
        body: body.to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_json_snapshot;

    #[test]
    fn proxy_request_deserialization() {
        let req: ProxyRequest = serde_json::from_str(
            r#"{"url": "https://example.com", "method": "POST", "headers": {"content-type": "application/json"}, "body": [1,2,3]}"#,
        )
        .unwrap();
        assert_eq!(req.url, "https://example.com");
        assert_eq!(req.method, "POST");
        assert_eq!(req.headers.get("content-type").unwrap(), "application/json");
        assert_eq!(req.body, Some(vec![1, 2, 3]));
    }

    #[test]
    fn proxy_request_deserialization_defaults() {
        let req: ProxyRequest = serde_json::from_str(r#"{"url": "https://example.com"}"#).unwrap();
        assert_eq!(req.url, "https://example.com");
        assert_eq!(req.method, "GET");
        assert!(req.headers.is_empty());
        assert!(req.body.is_none());
    }

    #[test]
    fn proxy_response_serialization() {
        let resp = ProxyResponse {
            status_code: 200,
            headers: [("content-type".to_string(), "text/plain".to_string())]
                .into_iter()
                .collect(),
            body: vec![104, 101, 108, 108, 111],
        };
        assert_json_snapshot!(resp, @r###"
        {
          "statusCode": 200,
          "headers": {
            "content-type": "text/plain"
          },
          "body": [
            104,
            101,
            108,
            108,
            111
          ]
        }
        "###);
    }
}
