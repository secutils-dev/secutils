---
title: Technology stack overview
description: "An updated tour of the Secutils.dev technology stack: Rust + Actix Web on PostgreSQL with Ory Kratos for identity, an embedded Deno JS runtime, Retrack for web scraping, and a React/EUI Web UI in a single mono-repo."
slug: technology-stack-overview
authors: azasypkin
image: https://secutils.dev/docs/img/blog/goal.png
tags: [overview, technology]
keywords: [secutils.dev tech stack, rust actix web, postgresql sqlx, ory kratos, deno javascript runtime, retrack web scraper, playwright chromium, react eui, docusaurus, utoipa openapi]
---

Hello!

Today, I'd like to provide an updated tour of the technology stack powering [**Secutils.dev**](https://secutils.dev), the open-source security toolbox for engineers and researchers. If you're considering similar choices for your own indie or open-source project, hopefully something here is useful. Let's dive in!

<!--truncate-->

:::info UPDATE (May 2026)
This post originally described a three-repo layout, a SQLite database, and an in-process search index based on Tantivy. The codebase has matured significantly since then and is now organized as a single [**mono-repo**](https://github.com/secutils-dev/secutils) backed by **PostgreSQL 16** and **Ory Kratos**, with a separate scheduling/scraping service called [**Retrack**](https://github.com/secutils-dev/retrack) included as a git submodule. The whole architecture is described in [**ARCHITECTURE.md**](https://github.com/secutils-dev/secutils/blob/main/ARCHITECTURE.md). The sections below have been rewritten to reflect this.
:::

<div class="text--center">
  <a href="/docs/blog/beta-release"><strong>🚀 Secutils.dev beta release is now public, click here to read more</strong></a>
</div>

---

**DISCLAIMER:** Some of the choices below may seem like overkill for a side project. As a solo engineer/founder, internal motivation matters as much as engineering pragmatism, so a stack I enjoy maintaining helps me ship over the long run. With that out of the way, here's the picture today.

---

## Repository layout

Secutils.dev lives in a single mono-repo at [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils). The high-level layout is:

```
secutils/
├── components/
│   ├── retrack/             # git submodule: scheduling + headless-browser scraper
│   ├── secutils-docs/       # documentation site (Docusaurus)
│   ├── secutils-jwt-tools/  # JWT generation CLI (Cargo workspace member)
│   └── secutils-webui/      # Web UI (React + TypeScript + Parcel)
├── dev/docker/              # Docker Compose files for local + e2e infrastructure
├── e2e/                     # Playwright end-to-end tests
├── migrations/              # SQLx database migrations
└── src/                     # Secutils API server (Rust)
```

The earlier split across `secutils`, `secutils-webui`, and `secutils-docs` repositories caused friction every time a change touched more than one component. Moving to a single repo lets me keep the API, UI, docs, and e2e tests in lockstep, and the [**ARCHITECTURE.md**](https://github.com/secutils-dev/secutils/blob/main/ARCHITECTURE.md) document gives a one-page overview of the moving parts.

## Backend (API server)

### Programming language: Rust

I have extensive experience in two languages: TypeScript and Rust. I could have stood up a functional MVP much faster in TypeScript and Node.js, but I deliberately chose Rust for the backend.

The usual benefits, memory safety and fearless concurrency, are noteworthy, but my primary motivation is more practical: in my experience, if a Rust program compiles, it tends to actually work. That property is invaluable for a small team striving for fast iteration with a low rate of trivial bugs, especially when shipping a developer-facing tool where bugs are particularly annoying.

Rust also excels at cross-compilation: I develop on `x86_64`, but the production deployment runs on cheaper ARM servers, and Cargo handles that smoothly.

### Web framework: Actix Web + utoipa

The API is built on [**Actix Web**](https://github.com/actix/actix-web) and is organized into ten tagged groups, each with its own base path:

| Tag            | Base path                                                           | What it covers                                                       |
|----------------|---------------------------------------------------------------------|----------------------------------------------------------------------|
| `webhooks`     | `/api/webhooks/responders`                                          | Webhook responders that capture, replay, and intercept HTTP requests |
| `certificates` | `/api/certificates/templates`, `/api/certificates/private_keys`     | X.509 certificate templates and private keys                         |
| `web_scraping` | `/api/web_scraping/page_trackers`, `/api/web_scraping/api_trackers` | Page trackers and API trackers (backed by Retrack)                   |
| `web_security` | `/api/web_security/csp`                                             | Build, parse, and serialize Content Security Policy headers          |
| `api_keys`     | `/api/user/api_keys`                                                | API keys for programmatic and agent access                           |
| `tags`         | `/api/user/tags`                                                    | Coloured tags shared across every utility                            |
| `secrets`      | `/api/user/secrets`                                                 | Encrypted user secrets referenced from scripts                       |
| `scripts`      | `/api/user/scripts`                                                 | Reusable JS/TS snippets for responders and trackers                  |
| `settings`     | `/api/user/settings`                                                | User preferences                                                     |
| `data`         | `/api/user/data`                                                    | Full data export and import                                          |

There are also unauthenticated `/api/status` and `/api/ui/*` endpoints (status probes and the Web UI bootstrap state) and the user-facing `*.webhooks.secutils.dev/*` subdomain that the API serves directly. The full list with request/response schemas is browsable at [**secutils.dev/api-docs**](https://secutils.dev/api-docs).

Every public route is annotated with [**utoipa**](https://github.com/juhaku/utoipa) so the OpenAPI specification is generated automatically from the Rust source and is published live at [**secutils.dev/api-docs/openapi.json**](https://secutils.dev/api-docs/openapi.json). The authoring conventions (path parameters, request body types, sync-guard tests for schema examples) are documented in [**AGENTS.md**](https://github.com/secutils-dev/secutils/blob/main/AGENTS.md#adding-a-new-http-route).

### Database: PostgreSQL 16 (via SQLx)

When the project started, a single SQLite file with [**Litestream**](https://github.com/benbjohnson/litestream) replication was perfectly adequate. As the data model grew (multi-tenant tags, user secrets, tracker history, request logs, OpenAPI metadata for the UI), the operational and concurrency story for SQLite became a constraint, so the API migrated to **PostgreSQL 16** in `1.0.0-beta.1` (May 2024).

I still talk to the database via the excellent [**SQLx**](https://github.com/launchbadge/sqlx) crate. SQLx verifies SQL queries at compile time, which has caught countless typos before they ever reached a test, and the `.sqlx/` cache in the repo keeps offline builds fast.

### Identity: Ory Kratos

Authentication and session management used to be hand-rolled inside Actix Web. Today they are delegated to [**Ory Kratos**](https://github.com/ory/kratos), an open-source identity service that handles registration, login, MFA, password recovery, and account verification flows. Kratos runs alongside the API, talks to the same PostgreSQL instance (under a separate schema), and exposes a self-service API that the Web UI consumes directly.

This freed me from owning a security-critical component (the auth subsystem) without giving up any control: Kratos is open-source and runs in the same Docker network.

### Embedded JavaScript runtime: Deno

A surprisingly large amount of Secutils.dev is user-defined JavaScript: webhook responder bodies, tracker extractor scripts, user scripts, MITM mutations, and so on. To execute them safely in-process, the API embeds the [**Deno**](https://github.com/denoland/deno) runtime via `deno_core`, which gives me a sandboxed V8 isolate per execution with strict resource limits (default heap size and execution-time caps are configurable per subscription tier).

I wrote about the design in [**"Building a Rust application with embedded JavaScript extensions"**](/blog/rust-application-with-js-extensions). The runtime now powers user scripts and secrets too, which were [**introduced later**](/docs/project/changelog/) and are documented under [**Platform → User scripts**](https://secutils.dev/docs/guides/platform/user_scripts).

### Logging: tracing

Structured logging is provided by the [**`tracing`**](https://github.com/tokio-rs/tracing) crate (it replaced the original `log` + `env_logger` setup). Spans capture per-request context and are emitted as JSON in production for ingestion by my monitoring pipeline.

### Tests

Rust's built-in `cargo test` runner handles the bulk of the work, complemented by [**Insta**](https://github.com/mitsuhiko/insta) snapshot tests that pin the OpenAPI spec, response shapes, and other large structured outputs. Insta brings the ergonomics of Jest snapshots to the Rust ecosystem and is genuinely a joy to use.

End-to-end coverage runs in [**Playwright**](https://playwright.dev/) against a full Docker Compose stack (`make e2e-up && make e2e-test`). The same Playwright harness regenerates the docs screenshots, with stability tooling that absorbs sub-pixel anti-aliasing jitter, normalises webhook subdomains, and pins timestamps so screenshots are byte-identical across runs.

## Web Scraper: Retrack

The biggest architectural change since the original post is that web scraping moved into a dedicated open-source project: [**Retrack**](https://github.com/secutils-dev/retrack). Retrack is included in the mono-repo as a git submodule at `components/retrack` and ships two services:

- **Retrack API** (Rust, port `7676`): manages page and API trackers, schedules execution via cron, stores revision history.
- **Retrack Web Scraper** (Node.js + Chromium, port `7272`): renders pages with **Playwright** and extracts content. It also supports the **Camoufox** stealth browser engine for sites that aggressively fingerprint headless Chromium.

Splitting Retrack out means scraping can scale independently from the API and that other projects can reuse the same scheduling/scraping engine. Inside Secutils.dev it powers the unified [**Page tracker**](https://secutils.dev/docs/guides/web_scraping/page) and [**API tracker**](https://secutils.dev/docs/guides/web_scraping/api) features.

## Frontend (Web UI)

The Web UI lives at [`components/secutils-webui/`](https://github.com/secutils-dev/secutils/tree/main/components/secutils-webui) and is a single-page React application written in TypeScript and bundled with [**Parcel**](https://parceljs.org/).

### UI framework: Elastic UI

I work for Elastic, where the team behind [**Elastic UI (EUI)**](https://eui.elastic.co/) is easily reachable. Familiarity is a big part of why I chose it, but EUI is also one of the most complete React component libraries out there, with great accessibility defaults, and dense data-grid support that maps perfectly to Secutils.dev's grid-heavy workflows.

The static promotional homepage (`/`) does **not** ship React, only static HTML and [**Tailwind CSS**](https://tailwindcss.com), so it stays small and fast.

### Recent UI additions

A few quality-of-life features that landed since the original post:

- Collapsible sidebar with persistent state.
- System-default dark mode.
- `Cmd/Ctrl-K` workspace search (replaces the original Tantivy-based plan; in practice an in-memory index is plenty for what users need to find).
- Full-screen Monaco script editor with example scripts.
- Auto-refresh in the responder requests grid.
- "Duplicate" action for every utility.
- Lazy-loaded editor flyouts and a Scripts tab loaded on demand to keep the initial bundle small.
- The HTTP client switched from `axios` to native `fetch`.

## Documentation

The docs at [**secutils.dev/docs**](https://secutils.dev/docs) are built with [**Docusaurus**](https://docusaurus.io/) from `components/secutils-docs/`. Docusaurus's MDX support, Mermaid integration, and search-engine-friendly defaults make it ideal for a documentation-heavy product.

To make the docs LLM-friendly, the site also publishes:

- [**`/llms.txt`**](https://secutils.dev/llms.txt): the **full** concatenated documentation in a single Markdown file.
- [**`/llms-index.txt`**](https://secutils.dev/llms-index.txt): a compact link index pointing at each guide.

Both are generated by the [`docusaurus-plugin-llms`](https://www.npmjs.com/package/docusaurus-plugin-llms) plugin during the build.

## Frequently asked questions

### Why Rust for a small SaaS?

For confidence, longevity, and cheap deployment. Compile-time guarantees catch a wide class of bugs before they reach production, the resulting binary is small and easy to cross-compile to ARM, and the Rust ecosystem around web services (`actix-web`, `sqlx`, `tracing`, `tokio`) has matured enormously.

### Why move from SQLite to PostgreSQL?

PostgreSQL handles concurrent writers, larger working sets, and relational features (e.g. foreign keys with cascading deletes across user data, tag joins, JSONB indexing) much more comfortably as the data model grows. Litestream is great for SQLite, but operationally PostgreSQL is also easier to back up and replicate at scale.

### Why Ory Kratos instead of rolling my own auth?

Auth is one of those areas where rolling your own is a long-term liability. Kratos is open-source, runs in the same Docker network, supports modern flows (passkeys, MFA, social sign-in), and lets me focus on the security utilities Secutils.dev is actually about.

### Why a separate Retrack service?

Headless-browser workloads have very different scaling, security, and sandboxing characteristics from a stateless HTTP API. Splitting Retrack out lets me apply tight resource limits, enable the Chromium sandbox, and run network policies that block access to internal IP ranges, without dragging the rest of the API into that model. See [**"Running web scraping service securely"**](/blog/running-web-scraping-service-securely) for the security story.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
