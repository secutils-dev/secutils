# <img src="https://raw.githubusercontent.com/secutils-dev/secutils/main/assets/logo/secutils-logo-initials.png" alt="Secutils.dev" width="22"> [Secutils.dev](https://secutils.dev) &middot; [![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://github.com/secutils-dev/secutils/blob/main/LICENSE) [![Build Status](https://github.com/secutils-dev/secutils/actions/workflows/ci.yml/badge.svg)](https://github.com/secutils-dev/secutils/actions)

Secutils.dev is an open-source, versatile, yet simple security toolbox for engineers and researchers built by
application security engineers.

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

Before running the Secutils.dev server, you need to configure the database and [Ory Kratos](https://github.com/ory/kratos) connections. If you don't have a PostgreSQL
and an Ory Kratos servers running, you [can run them locally with the following Docker Compose file:](https://docs.docker.com/language/rust/develop/)

```shell
docker-compose -f ./dev/docker/postgres-and-kratos.yml --env-file ./.env up --build --force-recreate
```

To remove everything and start from scratch, run:

```shell
docker-compose -f ./dev/docker/postgres-and-kratos.yml --env-file ./.env down --volumes --remove-orphans
```

Make sure to replace `POSTGRES_HOST_AUTH_METHOD=trust` in Docker Compose file with a more secure authentication method if you're
planning to use a local database for an extended period. For the existing database, you'll need to provide connection details in the
TOML configuration file as explained below.

Once all services are configured, you can start the Secutils.dev server with `cargo run`. By default, the
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

# Connection details for Ory Kratos services.
[components]
kratos_url = 'http://localhost:4433/'
kratos_admin_url = 'http://localhost:4434/'

# A list of preconfigured users. Once a user with the specified email signs up, 
# the server will automatically assign the user the specified handle and tier.
[security.preconfigured_users]
"admin@mydomain.dev" = { handle = "admin", tier = "ultimate" }

# The configuration of the Deno runtime used to run responder scripts.
[js_runtime]
max_heap_size = 10_485_760 # 10 MB
max_user_script_execution_time = 30_000 # 30 seconds

# SMTP server configuration used to send emails (signup emails, notifications etc.).
[smtp]
address = "xxx"
username = "xxx"
password = "xxx"

[utils]
webhook_url_type = "path"
```

If you saved your configuration to a file named `secutils.toml`, you can start the server with the following command:

```shell
cargo run -- -c secutils.toml
```

You can also use `.env` file to specify the location of the configuration file and database connection details required
for development and testing:

```dotenv
# Refer to https://github.com/launchbadge/sqlx for more details.
DATABASE_URL=postgres://postgres@localhost/secutils

# Path to the configuration file.
SECUTILS_CONFIG=${PWD}/secutils.toml

# Secret key used to sign and verify JSON Web Tokens for API access
# openssl rand -hex 16
SECUTILS_SECURITY__JWT_SECRET=8ffe0cc38d7ff1afa78b6cd5696f2e21

# JWT used by Kratos to authenticate requests to the API.
# Requires config: security.operators = ["@kratos"]
# Generated with: cargo run -p secutils-jwt-tools generate --secret 8ffe0cc38d7ff1afa78b6cd5696f2e21 --sub @kratos --exp 1year
SELFSERVICE_FLOWS_REGISTRATION_AFTER_PASSWORD_HOOKS_0_CONFIG_AUTH_CONFIG_VALUE="Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJleHAiOjE3NDcyMDExNTcsInN1YiI6IkBrcmF0b3MifQ.O506N__dZu7ZM6p-rEr_QkMn3jp0mRyBwKP7jstRHV8"
SELFSERVICE_FLOWS_REGISTRATION_AFTER_WEBAUTHN_HOOKS_0_CONFIG_AUTH_CONFIG_VALUE="Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJleHAiOjE3NDcyMDExNTcsInN1YiI6IkBrcmF0b3MifQ.O506N__dZu7ZM6p-rEr_QkMn3jp0mRyBwKP7jstRHV8"
COURIER_HTTP_REQUEST_CONFIG_AUTH_CONFIG_VALUE="Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJleHAiOjE3NDcyMDExNTcsInN1YiI6IkBrcmF0b3MifQ.O506N__dZu7ZM6p-rEr_QkMn3jp0mRyBwKP7jstRHV8"
```

### Web UI

Install all the required dependencies with `npm --prefix components/secutils-webui i` and run the UI in watch mode with `npm --prefix components/secutils-webui run watch`. The UI should be accessible at http://localhost:7171.

### Usage

At this point, it is recommended to use the Secutils.dev APIs through the Web UI, but you can also generate a JSON Web Token and use the 
APIs directly with `curl` or any other HTTP client. To generate a token, run the following command:

```shell
cargo run -p secutils-jwt-tools generate \
  --secret 8ffe0cc38d7ff1afa78b6cd5696f2e21 \
  --sub user@secutils.dev --exp 30days
---
eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJleHAiOjE3MTgyNjYxNTQsInN1YiI6InVzZXJAc2VjdXRpbHMuZGV2In0.e9sHurEyxhonOcR8dVVhmXdAWi287XReMiWUEVZuFwU
---
curl -XGET --header \
  "Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJleHAiOjE3MTgyNjYxNTQsInN1YiI6InVzZXJAc2VjdXRpbHMuZGV2In0.e9sHurEyxhonOcR8dVVhmXdAWi287XReMiWUEVZuFwU" \
  http://localhost:7070/api/status
```

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
docker build --tag secutils-webui:latest -f Dockerfile.webui .

# Cross-compile to ARM64 architecture
docker build --platform linux/arm64 --tag secutils-api:latest .
docker build --platform linux/arm64 --tag secutils-webui:latest -f Dockerfile.webui .

# Cross-compile to ARM64 musl architecture
docker build --platform linux/arm64 --tag secutils-api:latest -f Dockerfile.aarch64-unknown-linux-musl .
```

## Documentation

The documentation for Secutils.dev is located
in [github.com/secutils-dev/secutils-docs](https://github.com/secutils-dev/secutils-docs/) and hosted
at [secutils.dev/docs](https://secutils.dev/docs).

## Shoutouts

Secutils.dev wouldn't be possible without the following amazing projects and tools:

| Name                                                                                                  | Description                                                                                                                                                                                                                                                               |
|-------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| ![JetBrains logo](https://resources.jetbrains.com/storage/products/company/brand/logos/jetbrains.png) | JetBrains develops fantastic developer tools that I use daily to build Secutils.dev. While the products aren't open-source or free by default, they provide a generous free license for open-source project maintainers. [Check it out!](https://jb.gg/OpenSourceSupport) |
| ![Ory Kratos logo](https://raw.githubusercontent.com/ory/meta/master/static/logos/logo-kratos.svg)    | [Ory Kratos](https://github.com/ory/kratos) is an open-source alternative to Auth0, Okta, or Firebase with hardened security and PassKeys, SMS, OIDC, Social Sign In, MFA, FIDO, TOTP and OTP, WebAuthn, passwordless and much more.                                      |
| To be continued...                                                                                    |                                                                                                                                                                                                                                                                           |

## Community

- ❓ Ask questions on [GitHub Discussions](https://github.com/secutils-dev/secutils/discussions)
- 🐛 Report bugs on [GitHub Issues](https://github.com/secutils-dev/secutils/issues)
- 📣 Stay up to date on new features and announcements on [Twitter](https://twitter.com/secutils)
  or [Mastodon](https://fosstodon.org/@secutils)
