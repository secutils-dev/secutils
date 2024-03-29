# <img src="https://raw.githubusercontent.com/secutils-dev/secutils/main/assets/logo/secutils-logo-initials.png" alt="Secutils.dev" width="22"> [Secutils.dev](https://secutils.dev) &middot; [![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://github.com/secutils-dev/secutils/blob/main/LICENSE) [![Build Status](https://github.com/secutils-dev/secutils/actions/workflows/ci.yml/badge.svg)](https://github.com/secutils-dev/secutils/actions)

Secutils.dev is an open-source, versatile, yet simple security toolbox for engineers and researchers built by
application security engineers.

Refer to [secutils-dev/secutils-webui](https://github.com/secutils-dev/secutils-webui) for the web interface component
of Secutils.dev.

## Why Secutils.dev?

Big security solutions are impressive, but often too expensive, complex, and kind of overkill for us regular engineers.
On the other hand, there's a bunch of handy tools and scripts tackling specific security problems - they're simple and
affordable, but trying to juggle them is hard and messy. Secutils.dev aims to be the sweet spot between hefty solutions
and scattered tools. It's open, user-friendly, and your go-to toolbox filled with carefully selected utilities commonly
used in daily work, whether you're operating solo or part of a big team.

Secutils.dev adheres to [open security principles](https://en.wikipedia.org/wiki/Open_security) and offers:

* Guided experience for complex security concepts
* [Request responders](https://secutils.dev/docs/guides/webhooks) for rapid mocking of HTTP APIs and webhooks
* [Templates](https://secutils.dev/docs/guides/digital_certificates) for certificates and private keys to test
  cryptographic security protocols
* [Content Security Policy (CSP) management](https://secutils.dev/docs/guides/web_security/csp), enabling the import and
  creation of policies from scratch
* Tools for [web page resource scraping](https://secutils.dev/docs/guides/web_scraping/resources), content tracking, and
  more

![Secutils.dev Webhooks](https://github.com/secutils-dev/.github/blob/main/profile/webhooks.png?raw=true)

![Secutils.dev Web Scraping](https://github.com/secutils-dev/.github/blob/main/profile/web_scraping.png?raw=true)

![Secutils.dev Digital Certificates](https://github.com/secutils-dev/.github/blob/main/profile/digital_certificates.png?raw=true)

![Secutils.dev Web Security](https://github.com/secutils-dev/.github/blob/main/profile/web_security.png?raw=true)

## Getting started

Before running the Secutils.dev server, you need to configure the database connection. If you don't have a PostgreSQL
server running, you can run a local one with Docker:

```shell
docker run --rm -d \
  -v "$(pwd)"/.data:/var/lib/postgresql/data \
  -p 5432:5432 \
  --network secutils \
  --name secutils_db \
  -e POSTGRES_DB=secutils \
  -e POSTGRES_HOST_AUTH_METHOD=trust \
  postgres
```

Make sure to replace `POSTGRES_HOST_AUTH_METHOD=trust` with a more secure authentication method if you're planning to
use a local database for an extended period. For the existing database, you'll need to provide connection details in the
TOML configuration file as explained below.

Once the database connection is configured, you can start the Secutils.dev server with `cargo run`. By default, the
server will be accessible via http://localhost:7070. Use `curl` to verify that the server is up and running:

```shell
curl -XGET http://localhost:7070/api/status
---
{"version":"1.0.0-beta.1","level":"available"}
```

The server can be configured with a TOML configuration file. See the example below for a basic configuration:

```toml
port = 7070

[db]
name = 'secutils'
host = 'localhost'
port = 5432
username = 'postgres'
password = 'password'

# A session key used to encrypt session cookie. Should be at least 64 characters long. 
# For example, can be generated with `openssl rand -hex 32`
[security]
session-key = "a1a95f90e375d24ee4abb567c96ec3b053ceb083a4df726c76f8570230311c58"

# The configuration of the Deno runtime used to run responder scripts.
[js-runtime]
max-heap-size = 10_485_760 # 10 MB
max-user-script-execution-time = 30_000 # 30 seconds

# SMTP server configuration used to send emails (signup emails, notifications etc.).
[smtp]
address = "xxx"
username = "xxx"
password = "xxx"

# Defines a list of predefined Secutils.dev users.
[[security.builtin-users]]
email = "user@domain.xyz"
handle = "local"
password = "3efab73129f3d36e"
tier = "ultimate"

[utils]
webhook-url-type = "path"
```

If you saved your configuration to a file named `secutils.toml`, you can start the server with the following command:

```shell
cargo run -- -c secutils.toml
```

You can also use `.env` file to specify the location of the configuration file and database connection details required
for development and testing:

```dotenv
# Path to the configuration file.
SECUTILS_CONFIG=${PWD}/secutils.toml

# Refer to https://github.com/launchbadge/sqlx for more details.
DATABASE_URL=postgres://postgres@localhost/secutils
```

### Usage

At this point, it is recommended to use the Secutils.dev APIs through
the [Web UI](https://github.com/secutils-dev/secutils-webui).

### Re-initialize local database

To manage **development** database, you need to install
the [SQLx's command-line utility](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli):

```shell
cargo install --force sqlx-cli

# Drops, creates, and migrates the database referenced
# in the `DATABASE_URL` from the `.env` file.
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

The documentation for Secutils.dev is located
in [github.com/secutils-dev/secutils-docs](https://github.com/secutils-dev/secutils-docs/) and hosted
at [secutils.dev/docs](https://secutils.dev/docs).

## Community

- ❓ Ask questions on [GitHub Discussions](https://github.com/secutils-dev/secutils/discussions)
- 🐛 Report bugs on [GitHub Issues](https://github.com/secutils-dev/secutils/issues)
- 📣 Stay up to date on new features and announcements on [Twitter](https://twitter.com/secutils)
  or [Mastodon](https://fosstodon.org/@secutils)
