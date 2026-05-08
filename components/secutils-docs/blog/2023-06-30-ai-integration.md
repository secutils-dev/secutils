---
title: Security tools and AI are a perfect match
description: "Why AI integration is a natural fit for security tools, illustrated with a Secutils.dev proof-of-concept that asks an LLM to explain captured webhook requests, and a 2026 update on where the project is heading."
slug: ai-integration
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-06-30_auto_responders_chat_gpt.png
tags: [thoughts, technology]
keywords: [ai security tools, llm http request explainer, openai integration, security tooling ai, mcp servers, secutils.dev ai roadmap, llm-friendly documentation]
---

Hello!

It's an understatement to say there is a lot of hype around AI right now. It is being integrated into everything. The company I work for, Elastic, is part of the same wave with the [**Elasticsearch Relevance Engine (ESRE)**](https://www.elastic.co/search-labs/blog/articles/may-2023-launch-announcement) and the [**Elastic AI Assistant**](https://www.elastic.co/blog/introducing-elastic-ai-assistant). I'm usually skeptical of overhyped technology, but I have to admit AI is making genuine, lasting waves.

So a natural question for me is: would AI integration be useful to the users of [**Secutils.dev**](https://secutils.dev)? Let's explore that with a small proof-of-concept I built this week.

<!--truncate-->

:::info UPDATE (May 2026)
The "Explain this HTTP request" PoC shown in the video below was an early experiment and **was not shipped** as a built-in Secutils.dev feature. The thinking has matured since:

- Documentation is now LLM-first by design. The full Secutils.dev docs are concatenated into a single Markdown file at [**secutils.dev/llms.txt**](https://secutils.dev/llms.txt), with a compact index at [**secutils.dev/llms-index.txt**](https://secutils.dev/llms-index.txt). This makes it trivial for any LLM (hosted or local) to ground its answers in current product behaviour.
- The project's longer-term [**AI direction is sketched in the SafetyDetectives interview**](/blog/interview-safety-detectives): natural-language tracker creation ("track the latest Node.js vulnerabilities and notify me weekly"), AI-aware honeypots, and a future where security toolkits expose [**MCP**](https://modelcontextprotocol.io/) servers Secutils.dev can talk to. Local/offline LLMs are a deliberate first-class option, since hosted LLMs are often constrained on adversarial security topics.

The post below is preserved for context. Treat the OpenAI integration sketch as a future-direction PoC, not a shipped feature.
:::

For those who prefer a visual demo, here's the original video showing the experimental "Explain" action on a captured webhook request.

<video controls preload="metadata" width="100%">
  <source src="../video/blog/2023-06-30_auto_responders_chat_gpt.webm" type="video/webm" />
  <source src="../video/blog/2023-06-30_auto_responders_chat_gpt.mp4" type="video/mp4" />
</video>

:::tip NOTE
In this post I use the generic term "AI" without going into model specifics, for the argument here, that's enough.
:::

## The setup

In my previous post, [**Exploring third-party services with webhooks**](/blog/exploring-services-with-webhooks), I demonstrated how Secutils.dev's webhook responders can be used to understand how third-party services implement their features (in that example, dissecting the requests Notion makes when rendering a rich web bookmark). The raw HTTP records captured by a responder are dense and rich, but turning them into useful intelligence either requires real domain expertise or a lot of time spent looking up individual headers, vendors, and IP ranges. That's where an LLM can really help.

In the PoC, I wired up OpenAI so that you could pick a captured request and ask the model to "explain" it. For example:

```markdown
**HTTP method:**
HEAD

**HTTP headers:**
x-forwarded-for: 52.36.186.228
x-datadog-parent-id: 6049176225297304848
x-forwarded-proto: https
accept: */*
host: secutils.dev
user-agent: NotionEmbedder
x-datadog-sampling-priority: -1
referer: https://www.notion.so/
x-datadog-trace-id: 8895018898135420398
x-forwarded-host: secutils.dev
x-forwarded-port: 443
x-forwarded-server: azbyte-prod
x-real-ip: 52.36.186.228

**HTTP body:**
-
```

The model's output:

> Based on this information, we can make the following observations:
> 1. The request is a HEAD request, which is **typically used to retrieve only the headers of a resource without transferring the actual body**.
> 2. The request is sent to the host "secutils.dev".
> 3. The headers include information related to tracing and logging, such as `x-datadog-parent-id`, `x-datadog-trace-id`, and `x-datadog-sampling-priority`. **These headers suggest the usage of Datadog for tracing and monitoring purposes**.
> 4. The `x-forwarded-*` headers indicate that the **request has been forwarded from another server, possibly a load balancer or proxy**.
> 5. The `user-agent` header identifies the client as "NotionEmbedder", likely a **custom user agent** used by the client application.
> 6. The `accept` header indicates that the client can accept any type of response.
> 7. The `referer` header suggests that the **request was triggered from `https://www.notion.so/`**.

Not bad for a generic model with no fine-tuning. Imagine the difference with a model trained on HTTP traces and security advisories.

## Where AI fits in Secutils.dev

The PoC explains a single request. Even more interesting use cases come from chaining capabilities:

- Feed the model **multiple related requests** so it can reconstruct the whole conversation.
- Allow **follow-up questions** ("which of these IPs would you flag for further inspection?", "what's a likely reason for this header to disappear in the third request?").
- Let users ask the AI to **generate** responder bodies: "give me CORS-friendly headers", "generate an HTML form posting to `https://example.com/login`", or "produce a static page exposing `iframely:image` and `iframely:title`".
- Use AI to **summarise tracker diffs** ("the latest revision of this page added a new third-party script from a CDN you haven't seen before").
- Generate or refactor [**user scripts**](https://secutils.dev/docs/guides/platform/user_scripts) and tracker extractors via natural language.

## Why local LLMs matter for security work

Hosted models are great for casual use, but the safety boundaries baked into popular hosted LLMs often refuse or sanitise legitimate security questions. For tools used by actual security researchers, **local and offline LLMs** are an important part of the design space. Self-hostable models, especially the open-weight ones, run with whatever guardrails you choose, on data that never leaves your network. That's a much better fit for analysing traffic captured against your own infrastructure.

## Frequently asked questions

### Did the "Explain HTTP request" feature ship?

Not as a built-in feature. The PoC informed how AI is being thought about in Secutils.dev (see the [**SafetyDetectives interview**](/blog/interview-safety-detectives) for the longer arc) but the next round of AI work is more focused on natural-language tracker creation, MCP server integration, and offline-LLM support.

### Can I do this myself today?

Yes. The captured request log of any [**responder**](https://secutils.dev/docs/guides/webhooks) can be exported via the API, and you can pipe that into any LLM you have access to. The full docs are also published as a single Markdown file at [**`/llms.txt`**](https://secutils.dev/llms.txt), which makes Secutils.dev itself easy to ground an LLM on.

### Why focus on local/offline LLMs?

Hosted LLMs sometimes refuse legitimate security-research prompts. They also send the prompts you submit (potentially including sensitive request payloads) to a third party. Offline models avoid both problems and put the user in full control of how the model behaves.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
