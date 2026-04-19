//! `responder_like`: reproduces the real responder flow - a user script that
//! reads bytes out of `context.body`, does a small amount of JSON work, and
//! returns the Secutils `{ body, headers, statusCode }` envelope. Uses
//! `execute_script_with_body_conversion`, which wraps the script in the
//! runtime and normalises the returned body into a `Uint8Array` in Rust.
//!
//! This is the scenario most sensitive to Tier 2 #4 (moving the body
//! serialisation from JS back into Rust).

use crate::{
    measure::{Recorder, ScenarioResult, now},
    scenarios::common::{RESPONDER_JS, default_config},
};
use secutils::js_runtime::JsRuntime;
use serde::Deserialize;
use serde_bytes::ByteBuf;
use serde_json::json;

/// Mirrors Secutils' `ResponderScriptResult` shape just closely enough for
/// deserialisation to succeed: the harness only needs the round-trip, not
/// the actual field values.
#[derive(Deserialize)]
#[allow(dead_code)]
struct Envelope {
    #[serde(default)]
    body: Option<ByteBuf>,
    #[serde(default)]
    headers: Option<serde_json::Value>,
    #[serde(default, rename = "statusCode")]
    status_code: Option<u16>,
}

pub async fn run(iterations: u64, warmup: u64) -> anyhow::Result<ScenarioResult> {
    let context = build_context();

    for _ in 0..warmup {
        execute_once(RESPONDER_JS, &context).await?;
    }

    let mut recorder = Recorder::new(iterations, warmup)?;
    for _ in 0..iterations {
        let start = now();
        execute_once(RESPONDER_JS, &context).await?;
        recorder.observe(start.elapsed())?;
    }

    Ok(recorder.finalise())
}

async fn execute_once(script: &str, context: &str) -> anyhow::Result<()> {
    JsRuntime::execute_script::<Envelope>(
        default_config(),
        script.to_string(),
        Some(context.to_string()),
        None,
    )
    .await?;
    Ok(())
}

fn build_context() -> String {
    // 16 small items - a plausibly-sized webhook payload without being so
    // large that body serialisation dominates isolate startup.
    let items: Vec<_> = (0..16)
        .map(|i| json!({ "id": i, "value": i * 3 + 7, "label": format!("item-{i}") }))
        .collect();
    let body_text = serde_json::to_string(&json!({ "items": items })).unwrap();
    let body_bytes: Vec<u8> = body_text.as_bytes().to_vec();
    json!({
        "body": body_bytes,
        "headers": { "content-type": "application/json" },
        "method": "POST",
    })
    .to_string()
}
