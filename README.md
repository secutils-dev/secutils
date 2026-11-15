# <img src="https://raw.githubusercontent.com/secutils-dev/secutils/main/assets/logo/secutils-logo-initials.png" alt="Secutils.dev" width="22"> [Secutils.dev](https://secutils.dev) &middot; [![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://github.com/secutils-dev/secutils/blob/main/LICENSE) [![Build Status](https://github.com/secutils-dev/secutils/actions/workflows/ci.yml/badge.svg)](https://github.com/secutils-dev/secutils/actions)

Secutils.dev is an open-source security toolbox for engineers and researchers, built by application security engineers. Use the [hosted application](https://secutils.dev) or run it yourself.

## Why Secutils.dev?

Secutils.dev brings common security utilities into one interface, with guides for the concepts behind them. It serves engineers who need more than scattered scripts without the complexity of a large security platform.

Secutils.dev adheres to [open security principles](https://en.wikipedia.org/wiki/Open_security) and offers:

* [Request responders](https://secutils.dev/docs/guides/webhooks) to mock HTTP APIs, inspect webhook requests, and run custom JavaScript logic
* [Certificate templates](https://secutils.dev/docs/guides/digital_certificates/certificate_templates) and
  [private keys](https://secutils.dev/docs/guides/digital_certificates/private_keys) to test cryptographic protocols
* [Content Security Policy (CSP) management](https://secutils.dev/docs/guides/web_security/csp) to create, import, share, and test policies
* [Page trackers](https://secutils.dev/docs/guides/web_scraping/page) and
  [API trackers](https://secutils.dev/docs/guides/web_scraping/api) to monitor changes with scheduled checks and notifications

![Secutils.dev Webhooks](https://github.com/secutils-dev/.github/blob/main/profile/webhooks.png?raw=true)

![Secutils.dev Web Scraping](https://github.com/secutils-dev/.github/blob/main/profile/web_scraping.png?raw=true)

![Secutils.dev Digital Certificates](https://github.com/secutils-dev/.github/blob/main/profile/digital_certificates.png?raw=true)

![Secutils.dev Web Security](https://github.com/secutils-dev/.github/blob/main/profile/web_security.png?raw=true)

## Free single-page tools

These no-signup utilities cover common developer and security tasks. Browse the [tools catalog](https://tools.secutils.dev/).

| Tool | Description |
|------|-------------|
| [JWT Debugger](https://tools.secutils.dev/jwt) | Decode, verify, and sign HMAC JSON Web Tokens. |
| [SAML Decoder](https://tools.secutils.dev/saml) | Inspect SAML responses, requests, and metadata. |
| [PEM Certificate Decoder](https://tools.secutils.dev/pem) | Inspect PEM-encoded X.509 certificate chains. |
| [Markdown Preview](https://tools.secutils.dev/md) | Preview Markdown with diagrams, maps, 3D models, and math. Export HTML or PDF. |
| [MARS](https://mars.secutils.dev/) | Read, edit, and review Markdown with comments, a local document library, and HTML/PDF export. |
| [PDF Extractor](https://tools.secutils.dev/pdf) | Extract text and structured JSON from PDFs in the browser. |
| [HTTP Echo / Mock Response](https://tools.secutils.dev/echo) | Build a custom HTTP response and serve it through a shareable URL. |
| [Webhook Inspector](https://tools.secutils.dev/webhook) | Capture and inspect incoming HTTP requests through an ephemeral, encrypted webhook. |
| [Forecast](https://tools.secutils.dev/forecast) | Fit trendlines, forecast future values, and spot anomalies in numeric series. |

## Architecture

The project contains a Rust/Actix Web API, a React UI frontend, and Docusaurus documentation. [PostgreSQL](https://www.postgresql.org/) stores application data, and [Ory Kratos](https://www.ory.sh/kratos/) manages identity. [Retrack](https://github.com/secutils-dev/retrack), included as a Git submodule, runs page and API tracking.

[ARCHITECTURE.md](ARCHITECTURE.md) describes the services, repository layout, and local deployment.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
- [Node.js](https://nodejs.org/) at the version in [`.nvmrc`](.nvmrc)
- [Docker](https://docs.docker.com/get-docker/) and [Docker Compose](https://docs.docker.com/compose/install/)

## Getting Started

### 1. Clone the repository with submodules

```shell
git clone --recurse-submodules https://github.com/secutils-dev/secutils.git
cd secutils
```

If you cloned without `--recurse-submodules`, initialize the submodules:

```shell
git submodule update --init --recursive
```

### 2. Configure the environment

Copy the [example environment file](.env.example):

```shell
cp .env.example .env
```

Set `SECUTILS_SECURITY__JWT_SECRET` in `.env` to your own secret and generate a Kratos webhook token with the same secret:

```shell
cargo run -p secutils-jwt-tools -- generate \
  --secret "<your-jwt-secret>" --sub @kratos --exp 1year
```

Set the `SELFSERVICE_FLOWS_*` and `COURIER_*` values in `.env` to `Bearer <generated-token>`.

### 3. Start the infrastructure

Start PostgreSQL, Ory Kratos, and the Retrack services:

```shell
make dev-up
```

### 4. Start the Secutils API

```shell
make api
```

The default `SQLX_OFFLINE=true` uses cached query metadata for compilation. The API applies database migrations on startup.

The API runs at <http://localhost:7070>, make sure that it responds:

```shell
curl --fail http://localhost:7070/api/status
```

### 5. Start the Web UI

In another terminal, install the dependencies:

```shell
npm --prefix components/secutils-webui ci
```

Run the development server:

```shell
make webui
```

The Web UI runs at <http://localhost:7171>.

## Configuration

The API reads [secutils.toml](secutils.toml) and accepts environment overrides with the `SECUTILS_` prefix. Nested keys use `__`, for example `SECUTILS_DB__HOST=localhost`. [.env.example](.env.example) describes local configuration, authentication, and optional SMTP values.

## Development

Run `make help` for available commands and flags. [AGENTS.md](AGENTS.md) routes development tasks to their guides and lists the verification gates.

- [E2E tests](dev/agent-guides/e2e.md): Playwright tests against the full stack, plus standalone tests.
- [Dependency upgrades](dev/agent-guides/dependency-upgrades.md): coordinated upgrades and Retrack submodule revisions.
- [Docker image publishing](dev/agent-guides/image-publishing.md): build and publish images locally or through SSH.
- [Docs screenshots](dev/agent-guides/docs-screenshots.md): generate and maintain screenshots for the user guides.
- [Standalone tools](dev/tools/AGENTS.md): source, builds, and deployment.

**Data deletion:** `make dev-down`, `make e2e-down`, and `make db-reset` delete development data.

## Documentation

User guides are at [secutils.dev/docs](https://secutils.dev/docs). The [API reference](https://secutils.dev/api-docs) describes programmatic access, with
[API keys](https://secutils.dev/docs/guides/platform/api_keys) for scripts and automation.

To run the documentation site locally, install its dependencies:

```shell
npm --prefix components/secutils-docs ci
```

Run the development server:

```shell
make docs
```

The documentation site runs at <http://localhost:7373>, and [components/secutils-docs/README.md](components/secutils-docs/README.md) describes the documentation source and companion agent skills.

## Community

- ❓ Ask questions on [GitHub Discussions](https://github.com/secutils-dev/secutils/discussions)
- 🐛 Report bugs on [GitHub Issues](https://github.com/secutils-dev/secutils/issues)
- 🔒 Report vulnerabilities through the [security policy](SECURITY.md)
- 📣 Stay up to date on new features and announcements on [Twitter](https://twitter.com/secutils)
  or [Mastodon](https://fosstodon.org/@secutils)
