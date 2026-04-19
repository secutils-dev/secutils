//! Helpers shared by every scenario: config defaults, script fixtures, and a
//! permissive [`PublicUrlValidator`] that accepts anything pointing to the
//! local httpmock server.

use futures::future::BoxFuture;
use secutils::js_runtime::{JsRuntimeConfig, ProxyState, PublicUrlValidator};
use std::{sync::Arc, time::Duration};
use url::Url;

pub const TRIVIAL_JS: &str = include_str!("../../scripts/trivial.js");
pub const RESPONDER_JS: &str = include_str!("../../scripts/responder.js");
pub const PROXY_JS: &str = include_str!("../../scripts/proxy.js");

/// Realistic production-ish config: 10 MiB heap, 10 s wall-clock limit. These
/// match the defaults baked into the Secutils subscription config for a free
/// tier user running a responder script, so the numbers we record approximate
/// real production behaviour.
pub fn default_config() -> JsRuntimeConfig {
    JsRuntimeConfig {
        max_heap_size: 10 * 1024 * 1024,
        max_user_script_execution_time: Duration::from_secs(10),
    }
}

/// URL validator used by the `proxy_request` scenario. It unconditionally
/// approves any URL because we already restrict outgoing traffic to an
/// in-process httpmock server bound to 127.0.0.1.
#[derive(Clone)]
pub struct AllowAll;

impl PublicUrlValidator for AllowAll {
    fn is_public_web_url<'a>(&'a self, _url: &'a Url) -> BoxFuture<'a, bool> {
        Box::pin(futures::future::ready(true))
    }
}

pub fn proxy_state() -> ProxyState {
    ProxyState::new(
        Arc::new(AllowAll),
        false,
        10 * 1024 * 1024,
        Duration::from_secs(30),
    )
}
