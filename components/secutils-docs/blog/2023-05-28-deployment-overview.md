---
title: Deployment overview of micro-cluster for micro-SaaS
description: "How Secutils.dev is deployed today: a single mono-repo, mono-image-set Docker pipeline running on a self-hosted Kubernetes micro-cluster on Oracle Cloud, with Traefik, Let's Encrypt, PostgreSQL, Ory Kratos, and Retrack."
slug: deployment-overview
authors: azasypkin
image: https://secutils.dev/docs/img/blog/goal.png
tags: [overview, technology]
keywords: [secutils.dev deployment, kubernetes micro cluster, oracle cloud free tier, traefik ingress, let's encrypt, mono-repo docker images, ory kratos, retrack, micro-saas hosting]
---

Hello!

In [**my previous post**](/blog/2023-05-25-technology-stack-overview.md), I covered the technology stack behind [**Secutils.dev**](https://secutils.dev). Today I want to walk through how that stack is actually deployed: where the bits live, how they're networked, and how I've kept the operational footprint of a one-person SaaS small enough to manage in spare time.

<!--truncate-->

:::info UPDATE (May 2026)
The original post described a separate Git repository per component, each with its own `Dockerfile` and CI pipeline. The codebase has since been consolidated into a single [**mono-repo**](https://github.com/secutils-dev/secutils), and identity, web scraping, and database are now backed by **Ory Kratos**, **Retrack**, and **PostgreSQL 16** respectively. The deployment shape (Kubernetes on Oracle Cloud + Traefik + Let's Encrypt) is unchanged, but the components and Dockerfiles below have been updated to reflect the current layout.
:::

<div class="text--center">
  <a href="/docs/blog/beta-release"><strong>🚀 Secutils.dev beta release is now public, click here to read more</strong></a>
</div>

---

**DISCLAIMER:** A self-hosted Kubernetes cluster is overkill for a side-project SaaS by most reasonable measures. I run one because I enjoy it and because the Oracle Cloud free tier covers the bill, not because you have to. A boring `docker compose up` on a single VPS would work just as well for the traffic Secutils.dev sees today.

---

## URL layout

The simplified URL structure of the production deployment looks like this:

| Path                            | Audience  | What it serves                          |
|---------------------------------|-----------|-----------------------------------------|
| `secutils.dev/*`                | public    | Static promotional homepage             |
| `secutils.dev/docs/*`           | public    | Documentation site (Docusaurus)         |
| `secutils.dev/llms.txt`         | public    | Full docs concatenated for LLM crawlers |
| `secutils.dev/llms-index.txt`   | public    | Compact link index for LLM crawlers     |
| `secutils.dev/api-docs/*`       | public    | OpenAPI specification (utoipa)          |
| `secutils.dev/ws/*`             | private   | Web UI workspace (React SPA)            |
| `secutils.dev/api/*`            | mixed     | Secutils API (Actix Web)                |

## Components and Docker images

Even though the source now lives in a single mono-repo, the deployment is still broken into independent images so each can be sized and scaled separately:

- `secutils-api` (Rust + Actix Web): main HTTP API. Built from the root [`Dockerfile`](https://github.com/secutils-dev/secutils/blob/main/Dockerfile).
- `secutils-webui` (React SPA served by NGINX): built from [`Dockerfile.webui`](https://github.com/secutils-dev/secutils/blob/main/Dockerfile.webui).
- `secutils-docs` (Docusaurus build served by NGINX): built from [`Dockerfile.docs`](https://github.com/secutils-dev/secutils/blob/main/Dockerfile.docs).
- `retrack-api` (Rust): scheduling + tracker management, from the [Retrack submodule](https://github.com/secutils-dev/retrack).
- `retrack-web-scraper` (Node.js + Chromium): headless browser, from the [Retrack submodule](https://github.com/secutils-dev/retrack).
- **PostgreSQL 16**: managed via the official upstream image.
- **Ory Kratos**: managed via the official upstream image.

The two static-asset images (`secutils-webui`, `secutils-docs`) use **multi-stage builds** to keep the final layer to a NGINX Alpine base plus the built `dist/` directory. The pattern looks like this (excerpt):

```dockerfile
# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM node:22-alpine AS UI_BUILDER
WORKDIR /app
COPY components/secutils-webui/ .
RUN npm ci && npm run build

FROM nginx:stable-alpine
COPY --from=UI_BUILDER /app/dist/ /usr/share/nginx/html/
COPY components/secutils-webui/config/nginx.conf /etc/nginx/conf.d/default.conf
```

The Rust API image uses a **Debian distroless** runtime base for a small, hardened production image, with a `jemalloc` allocator linked in for better long-running memory behaviour. All base images are pinned by digest to prevent silent supply-chain shifts.

## Routing with Traefik + Let's Encrypt

The whole site sits behind a single [**Traefik**](https://traefik.io/) ingress, which dispatches requests to the right Kubernetes service based on the path or host. A simplified `IngressRoute` excerpt:

```yaml
apiVersion: traefik.io/v1alpha1
kind: IngressRoute
spec:
  routes:
    - kind: Rule
      match: Host(`secutils.dev`) && PathPrefix(`/api`)
      services:
        - kind: Service
          name: secutils-api-svc
          port: 7070
    - kind: Rule
      match: Host(`secutils.dev`) && PathPrefix(`/docs`)
      services:
        - kind: Service
          name: secutils-docs-svc
          port: 7373
```

TLS certificates the apex domain are issued and renewed automatically with [**Traefik's Let's Encrypt integration**](https://doc.traefik.io/traefik/https/acme/). The `.dev` TLD is on the [**HSTS preload list**](https://get.dev), so HTTPS is mandatory and a missed renewal would silently break the site, automation here is non-negotiable.

## Where it runs: Oracle Cloud free tier

The cluster runs on [Oracle Cloud's Always Free tier](https://www.oracle.com/cloud/free/) on ARM Ampere instances. ARM is a perfect fit for a Rust backend (cross-compilation is trivial with Cargo) and the Node.js scraper images run cleanly on `linux/arm64` too. I cover the cost story in [**"Running micro-SaaS for less than 1€ a month"**](/blog/running-micro-saas-for-less-than-one-euro-a-month).

## Local and e2e environments

The Docker Compose files under [`dev/docker/`](https://github.com/secutils-dev/secutils/tree/main/dev/docker) make it possible to bring up the entire production-equivalent stack locally:

```bash
make dev-up        # PostgreSQL + Kratos + Retrack (+ Web Scraper) for local API/UI dev
make dev-down

make e2e-up        # full stack incl. API + Web UI in Docker
make e2e-test      # run Playwright e2e tests
make e2e-down
```

The same e2e stack is what the Playwright test suite uses in CI to validate every PR end-to-end (see [**AGENTS.md**](https://github.com/secutils-dev/secutils/blob/main/AGENTS.md) for the full contract).

## Deployments

Deployments are still slightly old-school: I push images to a private Docker registry from the local `Makefile` (`make docker-api`, `make docker-webui`, `make docker-docs`), and the cluster pulls them via Argo CD. Most of the time I deploy to a `dev.secutils.dev` environment first for a smoke test, then promote to production.

Manual control over deployments suits a one-person project: I'd rather take the small operational tax than chase phantom auto-deploy regressions late at night.

## Frequently asked questions

### Do I need Kubernetes for a side-project SaaS?

No. Docker Compose on a single VPS would handle the current load comfortably. I run Kubernetes because I'm already familiar with it and because Oracle's free tier supports it. If you're starting fresh, optimise for what's boring and cheap.

### Why pin Docker base images by digest?

Pinning by tag (e.g. `node:22-alpine`) makes builds non-reproducible and exposes you to silent base-image rotations. Pinning by digest (`@sha256:...`) makes upgrades explicit and easy to audit.

### Where do I see what's actually deployed in production today?

The current API version is exposed at [`/api/status`](https://secutils.dev/api/status). The OpenAPI spec is at [`/api-docs/openapi.json`](https://secutils.dev/api-docs/openapi.json). The full architecture diagram lives in [**ARCHITECTURE.md**](https://github.com/secutils-dev/secutils/blob/main/ARCHITECTURE.md).

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
