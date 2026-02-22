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
* Tools for [web page content and resource tracking](https://secutils.dev/docs/guides/web_scraping/page), content tracking, and
  more

![Secutils.dev Webhooks](https://github.com/secutils-dev/.github/blob/main/profile/webhooks.png?raw=true)

![Secutils.dev Web Scraping](https://github.com/secutils-dev/.github/blob/main/profile/web_scraping.png?raw=true)

![Secutils.dev Digital Certificates](https://github.com/secutils-dev/.github/blob/main/profile/digital_certificates.png?raw=true)

![Secutils.dev Web Security](https://github.com/secutils-dev/.github/blob/main/profile/web_security.png?raw=true)

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain)
- [Node.js](https://nodejs.org/) 22+ (see `.nvmrc`)
- [Docker](https://docs.docker.com/get-docker/) and [Docker Compose](https://docs.docker.com/compose/install/)

## Getting Started

### 1. Clone the repository with submodules

```shell
git clone --recurse-submodules https://github.com/secutils-dev/secutils.git
cd secutils
```

If you already cloned without `--recurse-submodules`, initialize submodules with:

```shell
git submodule update --init --recursive
```

### 2. Set up the environment

Copy the example environment file and customize it:

```shell
cp .env.example .env
```

Generate JWT tokens for Kratos webhook authentication:

```shell
# Replace the secret with your own (openssl rand -hex 16)
cargo run -p secutils-jwt-tools -- generate \
  --secret <your-jwt-secret> --sub @kratos --exp 1year
```

Update the `SELFSERVICE_FLOWS_*` and `COURIER_*` values in `.env` with the generated token.

### 3. Start the infrastructure

Start PostgreSQL, Ory Kratos, Retrack API, and Retrack Web Scraper with Docker Compose:

```shell
make dev-up
```

Or directly:

```shell
docker compose -f dev/docker/docker-compose.yml --env-file .env up --build
```

To tear everything down and start fresh:

```shell
make dev-down
```

### 4. Start the Secutils API

```shell
cargo run
```

> **Note:** The `.env.example` ships with `SQLX_OFFLINE=true`, which tells sqlx to use the cached
> query metadata in `.sqlx/` instead of connecting to the database at compile time. This means you
> can compile and run without any manual migration step -- the app applies migrations automatically
> on startup. If you prefer live query validation during development, set `SQLX_OFFLINE=false` in
> `.env` and run `sqlx migrate run` first (requires
> [sqlx-cli](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli)).

The API will be available at http://localhost:7070. Verify it is running:

```shell
curl -s http://localhost:7070/api/status
# {"version":"1.0.0-beta.2","level":"available"}
```

### 5. Start the Web UI

```shell
npm --prefix components/secutils-webui i
npm --prefix components/secutils-webui run watch
```

The UI will be available at http://localhost:7171.

## Configuration

The server is configured with a TOML file (`secutils.toml`). See the example below:

```toml
port = 7070

[db]
name = 'secutils'
host = 'localhost'
port = 5432
username = 'postgres'
password = 'password'

[components]
kratos_url = 'http://localhost:4433/'
kratos_admin_url = 'http://localhost:4434/'

[retrack]
host = 'http://localhost:7676/'

[security.preconfigured_users]
"admin@mydomain.dev" = { handle = "admin", tier = "ultimate" }

[smtp]
address = "xxx"
username = "xxx"
password = "xxx"

[utils]
webhook_url_type = "path"
```

You can also override configuration values via environment variables with the `SECUTILS_` prefix
(nested keys use `__`, e.g. `SECUTILS_DB__HOST=localhost`).

## Updating the Retrack submodule

The [Retrack](https://github.com/secutils-dev/retrack) project is included as a git submodule at
`components/retrack`. To update it to the latest commit:

```shell
git submodule update --remote components/retrack
```

Or to pin to a specific commit:

```shell
cd components/retrack
git checkout <commit-hash>
cd ../..
git add components/retrack
```

## Documentation

Install dependencies and run the docs UI in watch mode:

```shell
npm --prefix components/secutils-docs i
npm --prefix components/secutils-docs run watch
```

The docs UI will be available at http://localhost:7373. Documentation is also hosted at
[secutils.dev/docs](https://secutils.dev/docs).

## End-to-End tests

E2E tests use [Playwright](https://playwright.dev/) and run against the full stack in Docker.

### Running locally

```shell
# Install Playwright and browsers (once)
cd e2e && npm ci && npx playwright install --with-deps chromium && cd ..

# Start the full stack
make e2e-up

# Wait for services to be ready, then run tests
make e2e-test

# Run in interactive UI mode
make e2e-test ARGS="--ui"

# Run in headed mode (visible browser)
make e2e-test ARGS="--headed"

# Run a specific test file
make e2e-test ARGS="tests/app.spec.ts"

# View the HTML report
make e2e-report

# Tear down
make e2e-down
```

### Usage with the API

Generate a JSON Web Token and use the APIs directly with `curl`:

```shell
cargo run -p secutils-jwt-tools -- generate \
  --secret <your-jwt-secret> --sub user@secutils.dev --exp 30days

curl -XGET --header \
  "Authorization: Bearer <generated-token>" \
  http://localhost:7070/api/status
```

## Re-initialize a local database

To manage the **development** database, install
[SQLx's command-line utility](https://github.com/launchbadge/sqlx/tree/main/sqlx-cli):

```shell
cargo install --force sqlx-cli

# Drops, creates, and migrates the database referenced
# in the DATABASE_URL from the .env file.
sqlx database drop
sqlx database create
sqlx migrate run
```

## Docker

Build images with the following commands:

```shell
# Host architecture
docker build --tag secutils-api:latest .
docker build --tag secutils-webui:latest -f Dockerfile.webui .
docker build --tag secutils-docs:latest -f Dockerfile.docs .

# Cross-compile to ARM64 architecture
docker build --platform linux/arm64 --tag secutils-api:latest .
docker build --platform linux/arm64 --tag secutils-webui:latest -f Dockerfile.webui .
docker build --platform linux/arm64 --tag secutils-docs:latest -f Dockerfile.docs .

# Cross-compile to ARM64 musl architecture
docker build --platform linux/arm64 --tag secutils-api:latest -f Dockerfile.aarch64-unknown-linux-musl .
```

## Available Make targets

| Command                      | Description                                                              |
|------------------------------|--------------------------------------------------------------------------|
| `make dev-up`                | Start dev infrastructure (`BUILD=1` to rebuild images)                   |
| `make dev-down`              | Stop dev infrastructure and remove volumes                               |
| `make dev-logs`              | Tail logs from dev infrastructure                                        |
| `make api`                   | Run the Secutils API (`cargo run`)                                       |
| `make webui`                 | Run the Web UI dev server                                                |
| `make docs`                  | Run the documentation dev server                                         |
| `make dev-debug-scraper`     | Start infra with web scraper routed to host (for headed browser)         |
| `make scraper-setup`         | Install web scraper npm dependencies (run once)                          |
| `make scraper`               | Run web scraper on host with visible browser (uses Chrome by default)    |
| `make e2e-up`                | Start the full e2e stack (`BUILD=1` to rebuild images)                   |
| `make e2e-down`              | Stop the e2e stack and remove volumes                                    |
| `make e2e-test`              | Run Playwright e2e tests (`ARGS="--ui"` for interactive mode)            |
| `make e2e-test-loop`         | Run e2e tests repeatedly (`RUNS=N` default 10, `ARGS=...`)               |
| `make e2e-report`            | Open the Playwright HTML report                                          |
| `make e2e-logs`              | Tail logs from the e2e stack                                             |
| `make docs-screenshots`      | Regenerate doc screenshots (requires e2e stack running, supports `ARGS`) |
| `make docs-screenshots-loop` | Run docs screenshot tests repeatedly (`RUNS=N` default 10, `ARGS=...`)   |
| `make db-reset`              | Drop, create, and migrate the dev database                               |
| `make docker-api`            | Build the Secutils API Docker image                                      |
| `make docker-webui`          | Build the Web UI Docker image                                            |
| `make docker-docs`           | Build the Docs Docker image                                              |
| `make clean`                 | Remove build artifacts                                                   |
| `make help`                  | Show all available targets                                               |

### Debugging the web scraper with a visible browser

To see Chromium while page trackers run, use the headed scraper mode instead of the
Docker-based one:

```shell
make scraper-setup         # once: install npm dependencies
make dev-debug-scraper     # start infra (scraper routed to host)
make scraper               # run web scraper with visible Chrome
```

By default `make scraper` uses Google Chrome on macOS. Override with:

```shell
make scraper CHROME_PATH="/path/to/chromium"
```

To switch back to the normal all-Docker setup: `make dev-down && make dev-up`.

## Shoutouts

Secutils.dev wouldn't be possible without the following amazing projects and tools:

| Name                                                                                                  | Description                                                                                                                                                                                                                                                               |
|-------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| ![Ory Kratos logo](https://raw.githubusercontent.com/ory/meta/master/static/logos/logo-kratos.svg)    | [Ory Kratos](https://github.com/ory/kratos) is an open-source alternative to Auth0, Okta, or Firebase with hardened security and PassKeys, SMS, OIDC, Social Sign In, MFA, FIDO, TOTP and OTP, WebAuthn, passwordless and much more.                                      |
| To be continued...                                                                                    |                                                                                                                                                                                                                                                                           |

## Community

- ❓ Ask questions on [GitHub Discussions](https://github.com/secutils-dev/secutils/discussions)
- 🐛 Report bugs on [GitHub Issues](https://github.com/secutils-dev/secutils/issues)
- 📣 Stay up to date on new features and announcements on [Twitter](https://twitter.com/secutils)
  or [Mastodon](https://fosstodon.org/@secutils)
