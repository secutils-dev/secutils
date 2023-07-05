# <img src="https://raw.githubusercontent.com/secutils-dev/secutils/main/assets/logo/secutils-logo-initials.png" alt="Secutils.dev" width="22"> [Secutils.dev](https://secutils.dev) &middot; [![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://github.com/secutils-dev/secutils/blob/main/LICENSE) [![Build Status](https://github.com/secutils-dev/secutils/actions/workflows/ci.yml/badge.svg)](https://github.com/secutils-dev/secutils/actions)

Secutils.dev is an open-source, versatile, yet simple toolbox for security-minded engineers.

Refer to [secutils-dev/secutils-webui](https://github.com/secutils-dev/secutils-webui) for the web interface component of Secutils.dev.

## Benefits

The main goal of this project is to provide security-minded engineers with a user-friendly, all-in-one toolbox for their day-to-day job that adheres to [open security principles](https://en.wikipedia.org/wiki/Open_security). You might want to consider Secutils.dev as a part of your usual development workflow for the following reasons:

* Built by application security engineer for security-minded engineers
* Carefully selected utilities that are commonly used in daily work
* Guided experience for complex security concepts
* Request bin, CSP builder, certificate generator, web scraper and more
* Intuitive and customizable user interface

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
cargo install sqlx-cli@0.7.0-alpha.3

# Drops, creates, and migrates the SQLite database
# referenced in the `DATABASE_URL` from the `.env` file.
sqlx database drop
sqlx database create
sqlx migrate run
```

## Documentation

The documentation for Secutils.dev is located in [github.com/secutils-dev/secutils-docs](https://github.com/secutils-dev/secutils-docs/) and hosted at [secutils.dev/docs](https://secutils.dev/docs).

## Community

- ❓ Ask questions on [GitHub Discussions](https://github.com/secutils-dev/secutils/discussions)
- 🐛 Report bugs on [GitHub Issues](https://github.com/secutils-dev/secutils/issues)
- 📣 Stay up to date on new features and announcements on [Twitter](https://twitter.com/secutils) or [Mastodon](https://fosstodon.org/@secutils)
