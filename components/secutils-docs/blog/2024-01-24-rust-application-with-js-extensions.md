---
title: "Supercharge your app with user extensions using Deno runtime"
description: "Supercharge your app with user extensions using Deno JavaScript runtime: embed JavaScript runtime to you Rust applications."
slug: rust-application-with-js-extensions
authors: azasypkin
image: /img/blog/2024-01-24_rust_application_with_js_extensions_execution_time.png
tags: [thoughts, overview, technology]
---
Hello!

Today, I'd like to discuss one of the many approaches to implement user extensions in your application, using [**"script" extensions for the webhooks**](/guides/webhooks#generate-a-dynamic-response) introduced in Secutils.dev in [**January, 2024 (1.0.0-alpha.5)**](https://github.com/secutils-dev/secutils/releases/tag/v1.0.0-alpha.5) as an example. In a nutshell, "script" extensions enable users to dynamically process incoming webhook requests and decide on the response on the fly, making simple webhooks akin to tiny applications.

As a user, have you ever wished for your favorite application to behave a little differently? Sometimes, even a slight change in behavior could make a big difference in the application or tool you rely on. Alternatively, as a developer, have you found yourself in a situation where numerous user feature requests seem almost identical but not quite enough to implement a single feature that satisfies all users without creating a ton of different toggles to customize behavior?

These are rhetorical questions, as I'm sure that such scenarios have crossed your path at least once. Otherwise, browser extensions, Shopify apps, Notion integrations, Grafana, and WordPress plugins wouldn't be as popular.

<!--truncate-->

As a solo-developer for [**Secutils.dev**](https://secutils.dev), I operate with very limited resources and cannot accommodate every user's feature request, even if I wish to. On the other hand, prioritizing and developing features based on assumptions and limited upfront user feedback has its own challenges and risks. That's why, right from the start, I've been considering adding some sort of "extension points" into Secutils.dev that would allow users to customize the certain behavior of the utilities according to their needs.

The core idea is that if a specific modification holds genuine value for the user, they wouldn't mind investing some time in extending the application themselves, provided they have the right tools and documentation. Actually, this serves as one of the most effective forms of validation that the feature is indeed necessary. Over time, validated user extensions make their way into the main application functionality or even community "extensions" marketplaces.


## Picking the extensions "framework"

The idea looks good in theory. Though it might not work for all applications or users, having a mostly developer audience makes things simpler. Developers are accustomed to modifications, plugins, and extensions. More importantly, they have coding skills, making them more comfortable with writing code to extend the applications they use. Moreover, with the emergence of highly capable code-generating language models (LLMs), being a developer might not be a strict requirement for crafting simple extensions in the future.

If I've convinced you that extending Sectuils.dev with the user code is a good idea, the next thing to consider is the language for this code. There are many great languages, but let's be honest — there's currently one universal "web" language, and that's JavaScript. It's easy to grasp and forgiving of user errors, making it the ideal language for user extensions!

If your application is written in JavaScript, integrating it with JavaScript extensions is a no-brainer. However, Secutils.dev is entirely written in Rust. How would I even begin? Fortunately, I recently came across an excellent blog post series explaining how to implement your JavaScript runtime in a Rust application with [**Deno**](https://deno.com/):

- [**Roll your own JavaScript runtime, Part 1**](https://deno.com/blog/roll-your-own-javascript-runtime)
- [**Roll your own JavaScript runtime, Part 2**](https://deno.com/blog/roll-your-own-javascript-runtime-pt2)
- [**Roll your own JavaScript runtime, Part 3**](https://deno.com/blog/roll-your-own-javascript-runtime-pt3)

Besides offering a JavaScript runtime, Deno also allows me to have complete control over which APIs and capabilities will be available to user JavaScript extensions. Brilliant!

## Using Deno Core as extensions runtime

:::info NOTE
I've left out some non-essential details in code examples for brevity, you can find the full source code on the [**Secutils.dev GitHub repository**](https://github.com/secutils-dev/secutils/blob/main/src/js_runtime.rs). I won't be explaining what Deno is and isn't in this blog post. If you're curious, you can find all the necessary information in the [**official Deno documentation**](https://deno.com/).
:::

The absolute minimum you need to embed a Deno JavaScript runtime in a Rust application is the [**`deno_core`**](https://crates.io/crates/deno_core) crate. The basic code to execute your extension, represented as a string with asynchronous JavaScript code, might look like this:

```rust
use deno_core::{
  JsRuntime, 
  serde_v8, 
  v8,
  PollEventLoopOptions,
  RuntimeOptions
};
use serde::Deserialize;

/// Executes a user script and returns the result.
pub async fn execute_script<R: for<'de> Deserialize<'de>>(
   js_code: impl Into<String>
) -> Result<R, anyhow::Error> {
    // Create a new instance of the JS runtime.
    let runtime = JsRuntime::new(RuntimeOptions::default());
    
    // Convert a JS code string to a `ModuleCodeString` and
    // retrieve the result. This snippet assumes that JS code
    // from `js_code` is asynchronous and returns `Promise`.
    // For example something along these lines: 
    // r#"(async () => {{ return 2 + 2; }})();"#
    let script_result_promise = runtime
        .execute_script("<anon>", js_code.into().into())?;

    // Now, wait for the promise to resolve.
    let resolve = runtime.resolve(script_result_promise);
    let script_result = runtime
        .with_event_loop_promise(
            resolve, 
            PollEventLoopOptions::default()
        )
        .await?;

    // Deserialize script result from v8 type and return.
    let scope = &mut runtime.handle_scope();
    let local = v8::Local::new(scope, script_result);
    serde_v8::from_v8(scope, local)
}
```

If you're familiar with Rust, the code should be self-explanatory: we take a string with JavaScript code, convert it to a type expected by Deno/V8, instruct the runtime to execute the script, wait for the result promise to resolve, and then extract and return the value.

It's also possible to supply parameters to the script being executed. There are various ways to do this, but I opted for the script global scope as a method of sharing input parameters with the script:

```rust
use deno_core::{serde_v8, v8};
use serde::Serialize;

// Make sure parameters can be serialized to a
// v8 compatible type.
#[derive(Serialize, Debug, PartialEq, Eq, Clone)]
struct ScriptParams {
    arg_num: usize,
    arg_str: String,
    arg_array: Vec<String>,
    arg_buf: Vec<u8>,
}

// Create params.
let script_params = ScriptParams {
    arg_num: 1,
    arg_str: "Hello, world!".to_string(),
    arg_array: vec!["one".to_string(), "two".to_string()],
    arg_buf: vec![1, 2, 3],
};

// Retrieve script "scope".
let scope = &mut runtime.handle_scope();
let context = scope.get_current_context();
let scope = &mut v8::ContextScope::new(scope, context);

// Prepare a key to store our params in the global scope.
let params_key = v8::String::new(scope, "param").unwrap();
// Serialize params value to a v8 compatible type.
let params_value = serde_v8::to_v8(scope, script_params)?;
// Set the value in the global scope (`globalThis.param`).
context
    .global(scope)
    .set(scope, params_key.into(), params_value); 
```

## Dealing with malfunctioning and malicious extensions

A JavaScript extension operating within a full-fledged JavaScript runtime is a powerful tool, and like any powerful tool, it can be quite harmful if not used correctly. When you're building an extension runtime that will run arbitrary user extensions, it's wise to operate under the assumption that it may be misused someday, whether intentionally malicious or not.

Fortunately, Deno Core already offers certain security assurances by default: user scripts cannot interact with the network and file system (unless you explicitly expose this functionality, and it's possible!). Even though it significantly reduces the potential for abuse or attacks, scripts can still consume all your CPU and memory resources, leading to a denial-of-service (DoS) for your application!

For instance, imagine a malicious user provides the following JavaScript extension that never completes and occupies your valuable server's resources:

```javascript
(() => {
    // Infinite loop.
    while (true) {}
})(); 
```

Typically, you should define a time limit for executing user extensions to handle long-running scripts (for Secutils.dev, it's set at 30 seconds), after which the "extension process" will be terminated. The code might look like this:

```rust
use std::{
    sync::{atomic::{AtomicBool, Ordering}, Arc},
    time::{Duration, Instant},
};

// Define a timeout after which the script will be terminated.
let termination_timeout = Duration::from_secs(30);

// Define the "cancellation token" that main thread
// can use to signal to the termination thread that
// script completed and termination isn't needed.
let timeout_token = Arc::new(AtomicBool::new(false));

// Retrieve v8::Isolate handle.
let isolate_handle = runtime.v8_isolate().thread_safe_handle();
let timeout_token_clone = timeout_token.clone();
std::thread::spawn(move || {
    let now = Instant::now();
    loop {
        // If main thread signaled that script completed
        // execution, exit.
        if timeout_token_clone.load(Ordering::Relaxed) {
            return;
        }

        // Otherwise, terminate execution if time is out, or sleep for max 2 sec.
        let Some(time_left) = termination_timeout.checked_sub(now.elapsed()) else {
            isolate_handle.terminate_execution();
            return;
        };

        std::thread::sleep(
            std::cmp::min(time_left, Duration::from_secs(2))
        );
    }
});

// Execute script...

// If the script completed execution early, tell
// "terminator" thread to exit.
timeout_token.swap(true, Ordering::Relaxed);
```

The code revolves around a special "terminator" thread that terminates the script execution when the time is up, and doesn't need additional explanation. The only detail worth mentioning is that I want the "terminator" thread to exit as early as possible if the script completes within the time budget. Hence, I check the status every 2 seconds instead of sleeping for 30 seconds.

Protecting against memory-hungry scripts in Deno is more challenging. I won't go into details about how it works and instead direct you to the issue in the Deno repository [**with all the details**](https://github.com/denoland/deno/issues/6916). In short, you need to create a JavaScript runtime with a specific heap limit and add a callback that's invoked when the memory limits are approached. This gives you a chance to terminate the execution before Deno/V8 crashes the entire process.

For example, a script like this would quickly consume all available memory:
```javascript
(async () => {{
   let s = "";
   while(true) { s += "Hello, World"; }
   return "Done";
}})();
```
And here’s how you can try to mitigate this:
```rust
use deno_core::{JsRuntime, RuntimeOptions};

// Create a new instance of the JS runtime with
// a 10 megabytes heap limit. 
let mut runtime = JsRuntime::new(RuntimeOptions {
    create_params: Some(
        v8::Isolate::create_params().heap_limits(0, 10 * 1024 * 1024),
    ),
    ..Default::default()
});

// Retrieve v8::Isolate handle and setup a "near_heap_limit" callback.
let isolate_handle = runtime.v8_isolate().thread_safe_handle();
runtime.add_near_heap_limit_callback(move |current_value, _| {
    // Terminate execution.
    isolate_handle.terminate_execution();

    // Give the runtime enough heap to terminate
    // without crashing the process.
    5 * current_value
});
```

These are good protective measures to have, but unfortunately, they don't provide complete protection. The script can quickly fill up memory, preventing termination from completing, or it might perform some heavy actions to hog your CPU. So, remember to set the CPU and memory limits for the container or Kubernetes pod where you're running your JavaScript runtime!

## Monitoring user extensions

If you're running user extensions or code, it's important to monitor them not only to alert you when something suddenly goes awry but also to gain valuable insights into how users extend and use your application.

As I mentioned in my [**"Privacy-friendly usage analytics and monitoring"**](./2023-05-30-usage-analytics-and-monitoring.md#monitoring) post, I rely on the [**Elastic Stack**](https://www.elastic.co/) to monitor Secutils.dev deployments. I use Filebeat and Metricbeat to collect and ingest application logs and metrics into Elasticsearch, which I can later use in my Kibana dashboards. I've created several visualizations and dashboards to monitor various aspects of my Secutils.dev Kubernetes deployment. Here are a few relevant to the "webhooks" script extensions:

### Script execution time

Firstly, I monitor how long user scripts take to execute. If a script takes more than 5 milliseconds to complete, it enters a "red zone" that makes me curious about what it does! The highest bar you see in the screenshot below is attributed to a script that renders PNG on the fly!

![Script execution time](/img/blog/2024-01-24_rust_application_with_js_extensions_execution_time.png)

### Script terminations and crashes

As I explained earlier in this post, I set limits on how much time (30 seconds) and memory (10 megabytes) a script can consume during execution. If a script exceeds these limits, it gets terminated, and the relevant logs are recorded for later review. This lets me understand the situation and decide what actions to take. If the user's intent is legitimate, I can collaborate with them individually to adjust these limits on a case-by-case basis. However, if the intent is malicious, well, I take some other measures 😬

![Script terminations and crashes](/img/blog/2024-01-24_rust_application_with_js_extensions_terminations.png)

### Overall memory consumption

As I integrate the Deno JavaScript runtime into a Secutils.dev API server application, I want to monitor the overall memory consumption of the API server. As shown in the following screenshot, the API server's memory consumption remains consistently low most of the time, especially when compared to the memory required by the [**Web Scraper**](/guides/web_scraping/content).

![Script terminations and crashes](/img/blog/2024-01-24_rust_application_with_js_extensions_memory.png)

All in all, I'm pleased with how it turned out and how straightforward it was to work with Deno Core. The "script" extensions have proven to be a nice way to turn static responders to a tiny applications that users can tailor to their needs without my involvement. I'm planning to make use of the Deno JavaScript runtime in other parts of Secutils.dev where I want to provide users with more flexibility. Stay tuned!

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).
:::
