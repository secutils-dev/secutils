---
title: Running micro-SaaS for less than 1€ a month
description: "How Secutils.dev runs in production for the cost of a domain name: GitHub Actions CI, Oracle Cloud Free Tier with ARM Ampere instances, self-hosted PostgreSQL, Plausible, Elastic Stack, Let's Encrypt, and Zoho Mail."
slug: running-micro-saas-for-less-than-one-euro-a-month
authors: azasypkin
image: https://secutils.dev/docs/img/blog/goal.png
tags: [overview, technology, economics]
keywords: [micro-saas hosting cost, oracle cloud free tier, github actions free tier, self-hosted postgresql, plausible self-hosted, elastic basic license, let's encrypt, zoho mail, indie hacker stack]
---

Hello!

In my previous posts I covered the [**technology stack**](/blog/2023-05-25-technology-stack-overview.md), the [**deployment process**](/blog/2023-05-28-deployment-overview.md), and the [**monitoring and analytics setup**](/blog/2023-05-30-usage-analytics-and-monitoring.md) behind [**Secutils.dev**](https://secutils.dev). Today, let's talk about money: what it actually costs to run this SaaS in production. As developers, we know the value of being resourceful and frugal, especially when bootstrapping a side project. Here's how the bill stays close to zero for Secutils.dev.

<!--truncate-->

:::info UPDATE (May 2026)
The cost story is essentially unchanged: the production deployment still runs for the price of the domain. A few line items have evolved:

- The codebase moved from three repositories to a single [**mono-repo**](https://github.com/secutils-dev/secutils), so CI is one workflow per concern (`ci.yml`, `ci-perf.yml`, `e2e.yml`) instead of three duplicated pipelines.
- The database migrated from SQLite (with Litestream replication to S3-compatible Object Storage) to **PostgreSQL 16**. Backups are now native PostgreSQL dumps stored on Oracle Cloud Object Storage.
- The web scraper is now a separate Rust + Node.js service called [**Retrack**](https://github.com/secutils-dev/retrack), included as a git submodule.
- The bullet list below has been refreshed to reflect the current shape. The underlying free tiers are the same.
:::

---

**DISCLAIMER:** The strategies below work well for early-stage products and micro-SaaS. They may not scale forever, but the failure mode of a side project is almost always "abandoned before it grew", not "couldn't keep up with growth". Optimise for boring and cheap until the data tells you otherwise.

---

## Source code management

**Cost:** 0€ / month

**Vendor:** [**GitHub**](https://github.com/pricing)

The source code for Secutils.dev is hosted in a single mono-repo at [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils), which is publicly available on GitHub. The few private bits (promotional website source, terms and privacy policy) live in private GitHub repositories. The free GitHub plan is more than enough, plus it includes secret scanning and Dependabot, both of which I happily use.

## Continuous integration

**Cost:** 0€ / month

**Vendor:** [**GitHub**](https://github.com/pricing)

CI runs on [**GitHub Actions**](https://github.com/features/actions), which provides 2,000 free minutes per month on the free plan. With everything in a single repo, I have a small set of workflows:

- `ci.yml`: builds and tests the Rust API, the Web UI, and the docs site, runs `cargo clippy` and `cargo test`, and validates that the OpenAPI spec snapshot is in sync.
- `ci-perf.yml`: runs the [**JS runtime performance harness**](https://github.com/secutils-dev/secutils/blob/main/AGENTS.md#js-runtime-performance-harness-benchesjs-runtime-perf) on every push to `main` and appends a record to `.perf/history.jsonl` only when something materially moves.
- `e2e.yml`: brings up the full Docker Compose stack and runs the [**Playwright e2e suite**](https://github.com/secutils-dev/secutils/tree/main/e2e).

Aggressive use of Cargo and `npm` caches keeps individual runs short. Average wall time for the Rust build is under 5 minutes thanks to `sccache` and `SQLX_OFFLINE=true` (cached query metadata in `.sqlx/`).

## Hosting

**Cost:** 0€ / month

**Vendor:** [**Oracle (Oracle Cloud Infrastructure)**](https://www.oracle.com/cloud/)

Secutils.dev runs on a self-managed Kubernetes micro-cluster on the [**Oracle Cloud Free Tier**](https://www.oracle.com/cloud/free/#always-free). The relevant offer:

> **Arm-based Ampere A1 cores and 24 GB of memory usable as 1 VM or up to 4 VMs with 3,000 OCPU hours and 18,000 GB hours per month.**

3,000 OCPU hours per month gives you 4 always-on OCPUs. I split that into a small cluster: a `secutils-prod` node (2 OCPUs, 12 GB RAM), a `secutils-dev` node (1 OCPU, 8 GB RAM), and a tiny `secutils-qa` node (1 OCPU, 4 GB RAM). All ARM, all `linux/arm64`, which suits the Rust API and the Node.js Retrack scraper equally well.

The free tier also covers unlimited inbound traffic and 10 TB of outbound traffic per month. Set up budget alerts so you notice immediately if anything starts running outside the free allowances.

## Monitoring

**Cost:** 0€ / month

**Vendor:** [**Elastic (self-hosted)**](https://www.elastic.co)

I run Elasticsearch, Kibana, Filebeat, and Metricbeat inside the same Kubernetes cluster, deployed via [**Elastic Cloud on Kubernetes (ECK)**](https://www.elastic.co/guide/en/cloud-on-k8s/current/k8s-quickstart.html) under the [**Basic license**](https://www.elastic.co/subscriptions). An [**index lifecycle policy**](https://www.elastic.co/guide/en/elasticsearch/reference/master/getting-started-index-lifecycle-management.html) keeps the on-disk footprint bounded by rolling over and deleting older indices.

For the structured-logging detail (the API switched to the [**`tracing`**](https://github.com/tokio-rs/tracing) crate), see [**"Privacy-friendly usage analytics and monitoring"**](/blog/usage-analytics-and-monitoring).

## Analytics

**Cost:** 0€ / month

**Vendor:** [**Plausible (self-hosted)**](https://plausible.io)

Plausible Analytics handles privacy-friendly product analytics, also self-hosted on the same Kubernetes cluster. Plausible stores events in ClickHouse, which compresses analytics data so well that storage will not be a concern any time soon.

## Database & backups

**Cost:** 0€ / month

**Vendor:** Self-hosted **PostgreSQL 16**, backups on **Oracle Cloud Object Storage**

PostgreSQL runs as a stateful set inside the cluster, backed by a block-volume PVC. Daily logical dumps (`pg_dump`) are pushed to Oracle Cloud Object Storage via the S3-compatible API. The 20 GB of Object Storage in the free tier is plenty for the current data volume.

The original SQLite + Litestream setup served the project well in 2023. PostgreSQL is the right call now that the data model has tags, tracker history, secrets, and per-user export/import to support.

## Secret management

**Cost:** 0€ / month

**Vendor:** [**Oracle (Oracle Cloud Infrastructure)**](https://www.oracle.com/cloud/)

Sensitive configuration (master keys, third-party API tokens, the Kratos JWT secret, etc.) lives in [**Oracle Cloud Vault**](https://docs.oracle.com/en-us/iaas/Content/KeyManagement/Concepts/keyoverview.htm), which is part of the free tier. HashiCorp Vault would also work, OCI Vault is just already there.

## TLS certificates

**Cost:** 0€ / month

**Vendor:** [**Internet Security Research Group (Let's Encrypt)**](https://letsencrypt.org)

[**Traefik with the Let's Encrypt provider**](https://doc.traefik.io/traefik/https/acme/) issues and renews certificates for `secutils.dev` automatically. The `.dev` TLD is on the [**HSTS preload list**](https://get.dev), so HTTPS is enforced and a missed renewal would silently break the site. Automation is non-negotiable here.

## Storage

**Cost:** 0€ / month

**Vendor:** [**Oracle (Oracle Cloud Infrastructure)**](https://www.oracle.com/cloud/)

200 GB of block volume storage is included in the free tier, which is more than enough for the current PostgreSQL volume, image registry caches, and Elastic indices. The 20 GB of Object Storage holds backups.

## Email hosting

**Cost:** 0€ / month

**Vendor:** [**Oracle (OCI Email Delivery)**](https://www.oracle.com/cloud/) and [**Zoho Mail**](https://www.zoho.com/mail/zohomail-pricing.html)

Transactional email (account activation, password reset, tracker change notifications) goes through OCI Email Delivery, which allows up to 3,000 emails per day on the free tier. Identity-related emails are sent by [**Ory Kratos**](https://github.com/ory/kratos), product notifications come from the in-house Rust subsystem (see [**"Q3 2023 update - Notifications"**](/blog/q3-2023-update-notifications)). Both use SMTP via [**Lettre**](https://github.com/lettre/lettre).

For personal mail from `*@secutils.dev` addresses, I use Zoho Mail's [**Forever Free Plan**](https://www.zoho.com/mail/zohomail-pricing.html).

## Marketing

**Cost:** 0€ / month

**Vendor:** Word of mouth, blog posts, niche communities

No paid ads, no influencer deals. I publish posts I believe people will find useful (this one included) and share them on my personal social channels and a few niche communities. The community has done a lot of the rest.

## Conclusion

The total cost of running [**Secutils.dev**](https://secutils.dev) in production is essentially the cost of the `secutils.dev` domain name: about 11.30€ per year, or roughly **0.94€ per month**. That is where the post title comes from.

There are startup credit programs that would cover a much fancier setup for a year or two, but they all come with strings attached and expire. For a bootstrapped indie SaaS, free-forever tiers compose into a stack that doesn't suddenly become expensive when the credits run out.

## Frequently asked questions

### Could you really host this on a single VPS?

Yes. The Kubernetes cluster is more elaborate than the project actually needs. A single ARM VPS with Docker Compose would handle the current load comfortably. I run Kubernetes because I'm familiar with it and because Oracle's free tier supports it.

### Will this stack survive growth?

Probably not in its exact shape, no. The PostgreSQL instance and Retrack scraper are the first components that would need vertical or horizontal scaling. At that point I'd happily start paying for managed PostgreSQL and dedicated scraper nodes. The point of the current setup is to keep the cost of "exists in production" as close to zero as possible.

### What about CDN?

Static assets are served by the in-cluster NGINX containers. At the current request volume there is no benefit to fronting them with a CDN. If the site ever takes huge organic traffic, Cloudflare's free tier in front of `secutils.dev` would solve that overnight.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
