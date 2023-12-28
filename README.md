# <img src="https://raw.githubusercontent.com/secutils-dev/secutils/main/assets/logo/secutils-logo-initials.png" alt="Secutils.dev" width="22"> [Secutils.dev](https://secutils.dev) &middot; [![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://github.com/secutils-dev/secutils/blob/main/LICENSE) [![Build Status](https://github.com/secutils-dev/secutils/actions/workflows/ci.yml/badge.svg)](https://github.com/secutils-dev/secutils/actions)

Secutils.dev is an open-source, versatile, yet simple toolbox for security-minded engineers built by application security engineers.

Refer to [secutils-dev/secutils-webui](https://github.com/secutils-dev/secutils-webui) for the web interface component of Secutils.dev.

![Secutils.dev UI](https://github.com/secutils-dev/.github/blob/main/profile/promo.png?raw=true)

## Why Secutils.dev?

Big security solutions are impressive, but often too expensive, complex, and kind of overkill for us regular engineers. On the other hand, there's a bunch of handy tools and scripts tackling specific security problems - they're simple and affordable, but trying to juggle them is hard and messy. Secutils.dev aims to be the sweet spot between hefty solutions and scattered tools. It's open, user-friendly, and your go-to toolbox filled with carefully selected utilities commonly used in daily work, whether you're operating solo or part of a big team.

Secutils.dev adheres to [open security principles](https://en.wikipedia.org/wiki/Open_security) and offers:
* Guided experience for complex security concepts
* [Request responders](https://secutils.dev/docs/guides/webhooks) for rapid mocking of HTTP APIs and webhooks
* [Templates](https://secutils.dev/docs/guides/digital_certificates) for certificates and private keys to test cryptographic security protocols
* [Content Security Policy (CSP) management](https://secutils.dev/docs/guides/web_security/csp), enabling the import and creation of policies from scratch
* Tools for [web page resource scraping](https://secutils.dev/docs/guides/web_scraping/resources), content tracking, and more

## Getting started

Before running the Secutils.dev server locally, you need to provide several required parameters. The easiest way is to specify them through a local `.env` file:
```dotenv
# An authenticated session key. For example, can be generated with `openssl rand -hex 32`
SECUTILS_SESSION_KEY=a1a95f90e375d24ee4abb567c96ec3b053ceb083a4df726c76f8570230311c58

# Defines a pipe-separated (`|`) list of predefined users in the following format: `email:password:role`.
SECUTILS_BUILTIN_USERS=user@domain.xyz:3efab73129f3d36e:admin

# Path to a local SQLite database file. Refer to https://github.com/launchbadge/sqlx for more details.
DATABASE_URL=sqlite:///home/user/.local/share/secutils/data.db
```

Once the .env file is created, you can start the Secutils.dev server with `cargo run`. By default, the server will be accessible via http://localhost:7070. Use `curl` to verify that the server is up and running:

```shellThis command 
curl -XGET http://localhost:7070/api/status
---
{"version":"1.0.0-alpha.1","level":"available"}
```

### Usage

At this point, it is recommended to use the Secutils.dev APIs through the [Web UI](https://github.com/secutils-dev/secutils-webui).

### Re-initialize local database

To manage the local SQLite database, you need to install the [SQLx's command-line utility](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli):
```shell
cargo install --force sqlx-cli

# Drops, creates, and migrates the SQLite database
# referenced in the `DATABASE_URL` from the `.env` file.
sqlx database drop
sqlx database create
sqlx migrate run
```

### Docker

Build images with the following commands:
```shell
# Host architecture
docker build --tag secutils-api:latest .

# Cross-compile to ARM64 architecture
docker build --platform linux/arm64 --tag secutils-api:latest .

# Cross-compile to ARM64 musl architecture
docker build --platform linux/arm64 --tag secutils-api:latest -f Dockerfile.aarch64-unknown-linux-musl .
```

## Documentation

The documentation for Secutils.dev is located in [github.com/secutils-dev/secutils-docs](https://github.com/secutils-dev/secutils-docs/) and hosted at [secutils.dev/docs](https://secutils.dev/docs).

## Community

- ❓ Ask questions on [GitHub Discussions](https://github.com/secutils-dev/secutils/discussions)
- 🐛 Report bugs on [GitHub Issues](https://github.com/secutils-dev/secutils/issues)
- 📣 Stay up to date on new features and announcements on [Twitter](https://twitter.com/secutils) or [Mastodon](https://fosstodon.org/@secutils)
