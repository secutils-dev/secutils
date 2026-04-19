//! `proxy_request`: exercises `op_proxy_request` against a local httpmock
//! server bound to 127.0.0.1. Every iteration reaches through the full path
//! (URL validation, `reqwest::Client` build, single HTTP round-trip).
//!
//! Tier 1 #2 (sharing a `reqwest::Client` across calls) should produce a
//! large drop in p50/p99 here, since today the client is rebuilt per
//! execution, re-initialising DNS and TLS state.

use crate::{
    measure::{Recorder, ScenarioResult, now},
    scenarios::common::{PROXY_JS, default_config, proxy_state},
};
use httpmock::prelude::*;
use secutils::js_runtime::JsRuntime;
use serde_json::json;

pub async fn run(iterations: u64, warmup: u64) -> anyhow::Result<ScenarioResult> {
    let server = MockServer::start_async().await;
    let _mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/echo");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"ok":true,"from":"httpmock"}"#);
        })
        .await;
    let url = format!("{}/echo", server.base_url());
    let context = json!({ "url": url }).to_string();

    for _ in 0..warmup {
        execute_once(&context).await?;
    }

    let mut recorder = Recorder::new(iterations, warmup)?;
    for _ in 0..iterations {
        let start = now();
        execute_once(&context).await?;
        recorder.observe(start.elapsed())?;
    }

    Ok(recorder.finalise())
}

async fn execute_once(context: &str) -> anyhow::Result<()> {
    JsRuntime::execute_script::<serde_json::Value>(
        default_config(),
        PROXY_JS.to_string(),
        Some(context.to_string()),
        Some(proxy_state()),
    )
    .await?;
    Ok(())
}
