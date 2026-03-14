use deno_core::{OpState, op2};
use deno_error::JsErrorBox;
use futures::future::BoxFuture;
use reqwest::redirect::Policy as RedirectPolicy;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashMap,
    io::Read as _,
    rc::Rc,
    sync::{Arc, OnceLock},
    time::Duration,
};
use url::Url;

/// Trait for validating whether a URL is publicly accessible (not pointing to
/// private/internal networks). Implemented by `Network<DR, ET>` so we can
/// erase the generic parameters for storage in [`ProxyState`].
pub trait PublicUrlValidator: Send + Sync {
    fn is_public_web_url<'a>(&'a self, url: &'a Url) -> BoxFuture<'a, bool>;
}

/// State injected into Deno's `OpState` before script execution.
///
/// Each script execution creates its own `ProxyState` (and thus its own
/// `reqwest::Client` instances).  This is critical because each script runs on
/// a dedicated `CurrentThread` tokio runtime that is dropped when the script
/// finishes.  A static/shared `reqwest::Client` would keep pooled connections
/// whose hyper background tasks are bound to the *previous* runtime, causing
/// "dispatch task is gone" errors on the next invocation.
#[derive(Clone)]
pub struct ProxyState {
    pub url_validator: Arc<dyn PublicUrlValidator>,
    pub restrict_to_public_urls: bool,
    pub max_response_size: usize,
    pub max_request_timeout: Duration,
    client: Arc<OnceLock<reqwest::Client>>,
    client_insecure: Arc<OnceLock<reqwest::Client>>,
}

impl ProxyState {
    pub fn new(
        url_validator: Arc<dyn PublicUrlValidator>,
        restrict_to_public_urls: bool,
        max_response_size: usize,
        max_request_timeout: Duration,
    ) -> Self {
        Self {
            url_validator,
            restrict_to_public_urls,
            max_response_size,
            max_request_timeout,
            client: Arc::new(OnceLock::new()),
            client_insecure: Arc::new(OnceLock::new()),
        }
    }

    fn client(&self, insecure: bool) -> &reqwest::Client {
        let cell = if insecure {
            &self.client_insecure
        } else {
            &self.client
        };
        cell.get_or_init(|| {
            let mut builder = reqwest::Client::builder().redirect(RedirectPolicy::none());
            if insecure {
                builder = builder.danger_accept_invalid_certs(true);
            }
            builder.build().expect("Failed to build proxy HTTP client")
        })
    }
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
    #[serde(default)]
    pub insecure: bool,
    pub timeout: Option<u64>,
    #[serde(default = "default_decompress")]
    pub decompress: bool,
}

fn default_decompress() -> bool {
    true
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

/// Decompresses the response body based on the `content-encoding` header.
/// On success, renames `content-encoding` to `x-original-content-encoding`
/// so scripts and clients know the body has been decoded.
fn decompress_body(
    mut headers: HashMap<String, String>,
    raw: &[u8],
) -> Result<(HashMap<String, String>, Vec<u8>), JsErrorBox> {
    let encoding = match headers.get("content-encoding") {
        Some(enc) => enc.trim().to_ascii_lowercase(),
        None => return Ok((headers, raw.to_vec())),
    };

    let decompressed = match encoding.as_str() {
        "gzip" | "x-gzip" => {
            let mut decoder = flate2::read::GzDecoder::new(raw);
            let mut buf = Vec::new();
            decoder.read_to_end(&mut buf).map_err(|e| {
                JsErrorBox::generic(format!("Failed to decompress gzip response body: {e}"))
            })?;
            buf
        }
        "deflate" => {
            let mut decoder = flate2::read::DeflateDecoder::new(raw);
            let mut buf = Vec::new();
            decoder.read_to_end(&mut buf).map_err(|e| {
                JsErrorBox::generic(format!("Failed to decompress deflate response body: {e}"))
            })?;
            buf
        }
        "br" => {
            let mut decoder = brotli::Decompressor::new(raw, 4096);
            let mut buf = Vec::new();
            decoder.read_to_end(&mut buf).map_err(|e| {
                JsErrorBox::generic(format!("Failed to decompress brotli response body: {e}"))
            })?;
            buf
        }
        "zstd" => zstd::stream::decode_all(raw).map_err(|e| {
            JsErrorBox::generic(format!("Failed to decompress zstd response body: {e}"))
        })?,
        // Identity or unknown encoding -- return as-is.
        _ => return Ok((headers, raw.to_vec())),
    };

    let original = headers.remove("content-encoding").unwrap();
    headers.insert("x-original-content-encoding".to_string(), original);

    Ok((headers, decompressed))
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

    let client = proxy.client(request.insecure);

    let max_timeout_ms = proxy.max_request_timeout.as_millis() as u64;
    let timeout = Duration::from_millis(
        request
            .timeout
            .unwrap_or(max_timeout_ms)
            .min(max_timeout_ms),
    );

    let mut req_builder = client.request(method, url).timeout(timeout);
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
        let mut chain = String::new();
        let mut source = std::error::Error::source(&e);
        while let Some(cause) = source {
            chain.push_str(" -> ");
            chain.push_str(&cause.to_string());
            source = std::error::Error::source(cause);
        }
        JsErrorBox::generic(if e.is_timeout() {
            format!("Upstream request timed out: {e}{chain}")
        } else if e.is_connect() {
            format!("Failed to connect to upstream: {e}{chain}")
        } else {
            format!("Upstream request failed: {e}{chain}")
        })
    })?;

    let status_code = response.status().as_u16();
    let headers: HashMap<String, String> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let raw_body = response
        .bytes()
        .await
        .map_err(|e| JsErrorBox::generic(format!("Failed to read upstream response body: {e}")))?;

    let (mut headers, body) = if request.decompress {
        decompress_body(headers, &raw_body)?
    } else {
        (headers, raw_body.to_vec())
    };

    if body.len() > proxy.max_response_size {
        return Err(JsErrorBox::generic(format!(
            "Upstream response body too large: {} bytes exceeds limit of {} bytes",
            body.len(),
            proxy.max_response_size
        )));
    }

    // Remove content-length since decompression (or even just reading the full
    // body) may have changed the size. Scripts can check body.length instead.
    if let Some(original_len) = headers.remove("content-length") {
        headers.insert("x-original-content-length".to_string(), original_len);
    }

    Ok(ProxyResponse {
        status_code,
        headers,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::GzEncoder;
    use insta::assert_json_snapshot;
    use std::io::Write;

    fn gzip_compress(data: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    fn deflate_compress(data: &[u8]) -> Vec<u8> {
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    fn brotli_compress(data: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut writer = brotli::CompressorWriter::new(&mut buf, 4096, 6, 22);
            writer.write_all(data).unwrap();
        }
        buf
    }

    fn zstd_compress(data: &[u8]) -> Vec<u8> {
        zstd::stream::encode_all(data, 3).unwrap()
    }

    fn make_headers(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

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
        assert!(!req.insecure);
        assert!(req.timeout.is_none());
        assert!(req.decompress);
    }

    #[test]
    fn proxy_request_deserialization_defaults() {
        let req: ProxyRequest = serde_json::from_str(r#"{"url": "https://example.com"}"#).unwrap();
        assert_eq!(req.url, "https://example.com");
        assert_eq!(req.method, "GET");
        assert!(req.headers.is_empty());
        assert!(req.body.is_none());
        assert!(!req.insecure);
        assert!(req.timeout.is_none());
        assert!(req.decompress);
    }

    #[test]
    fn proxy_request_deserialization_with_insecure_and_timeout() {
        let req: ProxyRequest = serde_json::from_str(
            r#"{"url": "https://example.com", "insecure": true, "timeout": 5000}"#,
        )
        .unwrap();
        assert_eq!(req.url, "https://example.com");
        assert!(req.insecure);
        assert_eq!(req.timeout, Some(5000));
    }

    #[test]
    fn proxy_request_deserialization_decompress_false() {
        let req: ProxyRequest =
            serde_json::from_str(r#"{"url": "https://example.com", "decompress": false}"#).unwrap();
        assert!(!req.decompress);
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

    #[test]
    fn decompress_body_gzip() {
        let original = b"Hello, compressed world!";
        let compressed = gzip_compress(original);
        let headers = make_headers(&[("content-encoding", "gzip"), ("content-type", "text/plain")]);

        let (result_headers, result_body) = decompress_body(headers, &compressed).unwrap();
        assert_eq!(result_body, original);
        assert_eq!(
            result_headers.get("x-original-content-encoding").unwrap(),
            "gzip"
        );
        assert!(!result_headers.contains_key("content-encoding"));
        assert_eq!(result_headers.get("content-type").unwrap(), "text/plain");
    }

    #[test]
    fn decompress_body_x_gzip() {
        let original = b"x-gzip variant";
        let compressed = gzip_compress(original);
        let headers = make_headers(&[("content-encoding", "x-gzip")]);

        let (result_headers, result_body) = decompress_body(headers, &compressed).unwrap();
        assert_eq!(result_body, original);
        assert_eq!(
            result_headers.get("x-original-content-encoding").unwrap(),
            "x-gzip"
        );
    }

    #[test]
    fn decompress_body_deflate() {
        let original = b"Deflate-compressed content";
        let compressed = deflate_compress(original);
        let headers = make_headers(&[("content-encoding", "deflate")]);

        let (result_headers, result_body) = decompress_body(headers, &compressed).unwrap();
        assert_eq!(result_body, original);
        assert_eq!(
            result_headers.get("x-original-content-encoding").unwrap(),
            "deflate"
        );
        assert!(!result_headers.contains_key("content-encoding"));
    }

    #[test]
    fn decompress_body_brotli() {
        let original = b"Brotli-compressed content";
        let compressed = brotli_compress(original);
        let headers = make_headers(&[("content-encoding", "br")]);

        let (result_headers, result_body) = decompress_body(headers, &compressed).unwrap();
        assert_eq!(result_body, original);
        assert_eq!(
            result_headers.get("x-original-content-encoding").unwrap(),
            "br"
        );
        assert!(!result_headers.contains_key("content-encoding"));
    }

    #[test]
    fn decompress_body_zstd() {
        let original = b"Zstd-compressed content";
        let compressed = zstd_compress(original);
        let headers = make_headers(&[("content-encoding", "zstd")]);

        let (result_headers, result_body) = decompress_body(headers, &compressed).unwrap();
        assert_eq!(result_body, original);
        assert_eq!(
            result_headers.get("x-original-content-encoding").unwrap(),
            "zstd"
        );
        assert!(!result_headers.contains_key("content-encoding"));
    }

    #[test]
    fn decompress_body_invalid_zstd_data() {
        let headers = make_headers(&[("content-encoding", "zstd")]);
        let result = decompress_body(headers, b"not valid zstd");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to decompress zstd")
        );
    }

    #[test]
    fn decompress_body_no_encoding_header() {
        let original = b"Plain body";
        let headers = make_headers(&[("content-type", "text/plain")]);

        let (result_headers, result_body) = decompress_body(headers, original).unwrap();
        assert_eq!(result_body, original);
        assert!(!result_headers.contains_key("x-original-content-encoding"));
        assert_eq!(result_headers.get("content-type").unwrap(), "text/plain");
    }

    #[test]
    fn decompress_body_identity_encoding() {
        let original = b"Identity body";
        let headers = make_headers(&[("content-encoding", "identity")]);

        let (result_headers, result_body) = decompress_body(headers, original).unwrap();
        assert_eq!(result_body, original);
        assert!(!result_headers.contains_key("x-original-content-encoding"));
        assert_eq!(result_headers.get("content-encoding").unwrap(), "identity");
    }

    #[test]
    fn decompress_body_unknown_encoding() {
        let original = b"Some body";
        let headers = make_headers(&[("content-encoding", "compress")]);

        let (result_headers, result_body) = decompress_body(headers, original).unwrap();
        assert_eq!(result_body, original);
        assert_eq!(result_headers.get("content-encoding").unwrap(), "compress");
    }

    #[test]
    fn decompress_body_case_insensitive() {
        let original = b"Case test";
        let compressed = gzip_compress(original);
        let headers = make_headers(&[("content-encoding", "  GZip  ")]);

        let (result_headers, result_body) = decompress_body(headers, &compressed).unwrap();
        assert_eq!(result_body, original);
        assert_eq!(
            result_headers.get("x-original-content-encoding").unwrap(),
            "  GZip  "
        );
    }

    #[test]
    fn decompress_body_invalid_gzip_data() {
        let headers = make_headers(&[("content-encoding", "gzip")]);
        let result = decompress_body(headers, b"not valid gzip");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to decompress gzip")
        );
    }

    #[test]
    fn decompress_body_empty_gzip() {
        let compressed = gzip_compress(b"");
        let headers = make_headers(&[("content-encoding", "gzip")]);

        let (_, result_body) = decompress_body(headers, &compressed).unwrap();
        assert!(result_body.is_empty());
    }

    #[test]
    fn decompress_body_large_json() {
        let json = serde_json::json!({
            "users": (0..100).map(|i| serde_json::json!({
                "id": i,
                "name": format!("User {i}"),
                "email": format!("user{i}@example.com"),
            })).collect::<Vec<_>>()
        });
        let original = serde_json::to_vec(&json).unwrap();
        let compressed = gzip_compress(&original);

        assert!(compressed.len() < original.len());

        let headers = make_headers(&[
            ("content-encoding", "gzip"),
            ("content-type", "application/json"),
        ]);

        let (_, result_body) = decompress_body(headers, &compressed).unwrap();
        assert_eq!(result_body, original);
    }
}
