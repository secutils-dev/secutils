---
title: "Supercharge your app with user extensions using Deno runtime"
description: "How Secutils.dev embeds the Deno (V8) JavaScript runtime in its Rust API to safely execute user JavaScript: heap and execution-time limits, V8 isolate termination, parameter passing via globalThis, and Kibana monitoring of script behaviour."
slug: rust-application-with-js-extensions
authors: azasypkin
image: /img/blog/2024-01-24_rust_application_with_js_extensions_execution_time.png
tags: [thoughts, overview, technology]
keywords: [embed deno in rust, deno_core, javascript runtime in rust, user scripts sandbox, v8 isolate termination, heap limit callback, webhook responder script, secutils.dev, mitm responder]
---

Hello!

Today, I'd like to discuss one approach to building **user extensions** into your application: embedding a JavaScript runtime in a Rust binary using [**Deno**](https://deno.com/). This was first introduced in Secutils.dev in [**v1.0.0-alpha.5 (January 2024)**](https://github.com/secutils-dev/secutils/releases/tag/v1.0.0-alpha.5) as ["script" extensions for webhook responders](/guides/webhooks#generate-a-dynamic-response). In a sentence: it lets users dynamically process every incoming webhook request and decide on the response on the fly, turning a static responder into a tiny app.

As a user, have you ever wished your favourite app would behave just slightly differently? As a developer, have you ever stared at twenty subtly-different feature requests, none of which justify a dedicated toggle? Plugins, extensions, and integrations are how products like Notion, Shopify, Grafana, and WordPress sidestep the problem. The pattern works for tiny SaaS too.

<!--truncate-->

:::info UPDATE (May 2026)
The model in this post is still the foundation of every place Secutils.dev runs user JavaScript, but the surface area has grown well beyond webhook responders. The same embedded Deno runtime now also powers:

- [**User scripts**](https://secutils.dev/docs/guides/platform/user_scripts): reusable JS/TS snippets that any utility can reference (great for centralising shared extraction or response logic).
- [**User secrets**](https://secutils.dev/docs/guides/platform/secrets): encrypted-at-rest values that scripts can reference by name without ever seeing the secret in plaintext.
- [**MITM responders**](https://secutils.dev/docs/guides/webhooks): script-driven mutation of proxied requests and responses, with a per-request response history.
- Tracker [**extractor scripts**](https://secutils.dev/docs/guides/web_scraping/page) (the JavaScript that runs inside the page during a Page tracker check, Playwright codegen output can be imported directly).

The runtime configuration shown in this post (10 MB heap, 30 s wall-clock limit) is still the baseline. Secutils.dev now also has a [**JS runtime performance harness**](https://github.com/secutils-dev/secutils/tree/main/benches/js-runtime-perf) that records per-PR latency / throughput / RSS regressions in CI. The original `delay` setting on responders was removed in this same release, it's a one-line user script now.
:::

As a solo developer, I can't build every feature anyone asks for, even when I want to. So Secutils.dev was always going to need extension points: hooks where users could shape behaviour without me. The bonus: if a user is willing to write code to extend the app, that's the strongest form of validation that the feature actually matters.

## Picking the extensions language

If your app is written in JavaScript, embedding a JavaScript extension language is a no-brainer. Secutils.dev is a Rust application, so the question is sharper. After looking at the alternatives (Lua, WebAssembly with WASI, Rhai), JavaScript still wins on familiarity, ecosystem, and forgiving syntax. And these days, even non-developers can produce a working JavaScript extension by asking an LLM.

The Deno team has an excellent three-part series on doing exactly this:

- [**Roll your own JavaScript runtime, Part 1**](https://deno.com/blog/roll-your-own-javascript-runtime)
- [**Roll your own JavaScript runtime, Part 2**](https://deno.com/blog/roll-your-own-javascript-runtime-pt2)
- [**Roll your own JavaScript runtime, Part 3**](https://deno.com/blog/roll-your-own-javascript-runtime-pt3)

The big bonus on top of "JavaScript inside a Rust binary" is that with Deno I get to choose **which APIs and capabilities** are exposed to user code. By default, no network, no filesystem.

## Using `deno_core` as the extensions runtime

:::info NOTE
I've trimmed non-essential details for brevity. The full source is in the Secutils.dev repo at [`src/js_runtime.rs`](https://github.com/secutils-dev/secutils/blob/main/src/js_runtime.rs). I won't re-explain Deno, the [**Deno docs**](https://deno.com/) are a great reference.
:::

The minimum needed to embed Deno's runtime in a Rust application is the [**`deno_core`**](https://crates.io/crates/deno_core) crate. The basic shape of "execute a string of JavaScript and read back the result":

```rust
use deno_core::{
    JsRuntime,
    serde_v8,
    v8,
    PollEventLoopOptions,
    RuntimeOptions,
};
use serde::Deserialize;

/// Executes a user script and returns the deserialized result.
pub async fn execute_script<R: for<'de> Deserialize<'de>>(
    js_code: impl Into<String>,
) -> Result<R, anyhow::Error> {
    let runtime = JsRuntime::new(RuntimeOptions::default());

    // Assume the script is async and returns a Promise, e.g.
    // `(async () => { return 2 + 2; })();`
    let script_result_promise = runtime
        .execute_script("<anon>", js_code.into().into())?;

    let resolve = runtime.resolve(script_result_promise);
    let script_result = runtime
        .with_event_loop_promise(
            resolve,
            PollEventLoopOptions::default(),
        )
        .await?;

    let scope = &mut runtime.handle_scope();
    let local = v8::Local::new(scope, script_result);
    serde_v8::from_v8(scope, local)
}
```

That's the whole pipeline: convert the string to V8's representation, execute it, await the resulting `Promise`, pull the value back into a Rust type.

Passing parameters into the script can be done a few ways. I picked **the script's global scope**, since it's both familiar and easy to type-check from Rust:

```rust
use deno_core::{serde_v8, v8};
use serde::Serialize;

#[derive(Serialize, Debug, PartialEq, Eq, Clone)]
struct ScriptParams {
    arg_num: usize,
    arg_str: String,
    arg_array: Vec<String>,
    arg_buf: Vec<u8>,
}

let script_params = ScriptParams {
    arg_num: 1,
    arg_str: "Hello, world!".to_string(),
    arg_array: vec!["one".to_string(), "two".to_string()],
    arg_buf: vec![1, 2, 3],
};

let scope = &mut runtime.handle_scope();
let context = scope.get_current_context();
let scope = &mut v8::ContextScope::new(scope, context);

let params_key = v8::String::new(scope, "param").unwrap();
let params_value = serde_v8::to_v8(scope, script_params)?;
context.global(scope).set(scope, params_key.into(), params_value);
```

The user script can then read input as `globalThis.param`.

## Defending against malfunctioning and malicious extensions

A JavaScript runtime that executes arbitrary user code is, by definition, a Powerful Tool. Build for the assumption that it will be misused. `deno_core` is good defence to start with: scripts cannot touch the network or filesystem unless you explicitly expose those capabilities. But scripts can still burn CPU and RAM. A trivial denial-of-service:

```javascript
(() => {
    while (true) {}
})();
```

The standard mitigation is a **wall-clock timeout** that terminates the V8 isolate. Secutils.dev sets it to 30 seconds:

```rust
use std::{
    sync::{atomic::{AtomicBool, Ordering}, Arc},
    time::{Duration, Instant},
};

let termination_timeout = Duration::from_secs(30);
let timeout_token = Arc::new(AtomicBool::new(false));
let isolate_handle = runtime.v8_isolate().thread_safe_handle();
let timeout_token_clone = timeout_token.clone();

std::thread::spawn(move || {
    let now = Instant::now();
    loop {
        if timeout_token_clone.load(Ordering::Relaxed) {
            return;
        }
        let Some(time_left) = termination_timeout.checked_sub(now.elapsed()) else {
            isolate_handle.terminate_execution();
            return;
        };
        std::thread::sleep(std::cmp::min(time_left, Duration::from_secs(2)));
    }
});

// ... execute script ...

// Tell the watchdog the script finished early.
timeout_token.swap(true, Ordering::Relaxed);
```

The watchdog wakes every 2 seconds (rather than sleeping the full 30) so that fast-finishing scripts release the watchdog thread quickly.

Memory exhaustion is harder. The standard pattern (described in [**denoland/deno#6916**](https://github.com/denoland/deno/issues/6916)) is to give V8 a hard heap limit and a "near limit" callback that terminates the isolate before V8 crashes the entire process. A script like this:

```javascript
(async () => {
    let s = '';
    while (true) { s += 'Hello, World'; }
    return 'Done';
})();
```

…can be defused with:

```rust
use deno_core::{JsRuntime, RuntimeOptions};

let mut runtime = JsRuntime::new(RuntimeOptions {
    create_params: Some(
        v8::Isolate::create_params().heap_limits(0, 10 * 1024 * 1024),
    ),
    ..Default::default()
});

let isolate_handle = runtime.v8_isolate().thread_safe_handle();
runtime.add_near_heap_limit_callback(move |current_value, _| {
    isolate_handle.terminate_execution();
    // Give the runtime enough headroom to terminate without
    // V8 panicking the host process.
    5 * current_value
});
```

This is much better than nothing, but it isn't a complete defence. A script can still race the watchdog or wedge the CPU. Always set explicit CPU/memory **container** limits at the orchestration layer (Kubernetes pod, systemd unit, docker `--memory`/`--cpus`) so the worst case is a single process restart.

## Monitoring user extensions

If you run user code in production, monitor it. Not just to catch bad scripts, but to learn how users actually extend the app. Secutils.dev ships logs and metrics into Elasticsearch via Filebeat/Metricbeat (see [**"Privacy-friendly usage analytics and monitoring"**](/blog/usage-analytics-and-monitoring)) and exposes the relevant signals as Kibana dashboards:

### Script execution time

If a script takes more than a few milliseconds, it warrants curiosity. The tallest bar in this dashboard belongs to a script that renders a PNG on the fly:

![Kibana dashboard showing webhook script execution time distribution](/img/blog/2024-01-24_rust_application_with_js_extensions_execution_time.png)

There's also a [**dedicated JS runtime performance harness**](https://github.com/secutils-dev/secutils/tree/main/benches/js-runtime-perf) (`benches/js-runtime-perf/`) that runs in CI on every push to `main`. It records p50/p99 latency, throughput, and peak RSS for cold-start, steady-state, responder-like, proxy-request, and burst workloads, and warns on regressions. Numbers are appended to `.perf/history.jsonl` only when something materially moves.

### Script terminations and crashes

Whenever a script hits the 30 s or 10 MB limit it's terminated and logged. Legitimate use cases get more headroom on a case-by-case basis, abusive ones get other treatment.

![Kibana dashboard showing script terminations and crashes by reason](/img/blog/2024-01-24_rust_application_with_js_extensions_terminations.png)

### Overall API memory consumption

The Rust API's memory footprint stays comfortably small even with heavy use of the runtime. The `jemalloc` allocator and the Debian distroless runtime image both help here. The Web Scraper container is the actual heavyweight in the deployment.

![Kibana dashboard showing API server memory consumption over time](/img/blog/2024-01-24_rust_application_with_js_extensions_memory.png)

## Where this primitive shows up today

The original webhook responder script was the first place Deno was wired in. Since then, the same runtime now also drives:

- [**MITM responders**](https://secutils.dev/docs/guides/webhooks) (intercept and rewrite proxied requests/responses).
- Tracker [**extractor scripts**](https://secutils.dev/docs/guides/web_scraping/page) and the rest of the [**Page tracker**](https://secutils.dev/docs/guides/web_scraping/page) / [**API tracker**](https://secutils.dev/docs/guides/web_scraping/api) extensibility surface.
- Reusable [**user scripts**](https://secutils.dev/docs/guides/platform/user_scripts) shared across every utility.
- Encrypted [**user secrets**](https://secutils.dev/docs/guides/platform/secrets) referenced from any of the above.

Adding a single embedded runtime turned out to be one of the highest-leverage architectural decisions in the project.

## Frequently asked questions

### Why Deno and not Node.js?

`deno_core` is purpose-built for embedding into other applications, with explicit control over which capabilities user code gets. Embedding the full Node.js runtime would drag in far more surface area than I want exposed to user scripts.

### Can user scripts make network requests?

Only via the curated APIs Secutils.dev exposes (e.g. the `op_proxy_request` op for outbound HTTP, with timeouts and per-tier limits). Raw `fetch` is not available by default, so a script can't open arbitrary connections to your internal network.

### What if my user wants more than 10 MB of heap or 30 seconds?

These are baseline defaults. Subscription tiers can lift them, and individual cases can be handled by hand. The ceiling is informed by the per-call latency and RSS numbers measured by the perf harness; we don't raise limits without knowing the cost.

### Where can I see the runtime's perf numbers over time?

The append-only history is in `.perf/history.jsonl` in the [**mono-repo**](https://github.com/secutils-dev/secutils/blob/main/.perf/history.jsonl), and there's a small standalone HTML report at `scripts/perf-report.html`.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).
:::
