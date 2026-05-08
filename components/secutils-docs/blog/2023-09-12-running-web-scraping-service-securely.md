---
title: "Running web scraping service securely"
description: "End-to-end security checklist for running a web scraping service: input validation, resource isolation, non-root containers, Chromium sandbox, seccomp profiles, Kubernetes NetworkPolicy egress allow-lists, and monitoring. Backed by the Retrack scraper used in Secutils.dev."
slug: running-web-scraping-service-securely
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-09-12_running_web_scraping_service_securely.png
tags: [overview, technology, application-security]
keywords: [web scraping security, ssrf prevention, chromium sandbox, seccomp profile, kubernetes network policy, non-root container, playwright security, retrack, secutils.dev]
---

Hello!

In an [**earlier post**](/blog/q3-2023-update-notifications) I talked about the notifications subsystem in [**Secutils.dev**](https://secutils.dev). Around the same time I was preparing to allow Secutils.dev users to inject custom JavaScript into the web pages they track resources for, which forced a serious round of security hardening on the [**Web Scraper**](https://github.com/secutils-dev/retrack/tree/main/components/retrack-web-scraper). This post is the result: an end-to-end checklist for anyone running a service that scrapes arbitrary user-supplied URLs.

<!--truncate-->

:::info UPDATE (May 2026)
This post still reflects the current security model, with a few naming updates:

- The web scraper used to be called **secutils-web-scraper**. It is now part of the standalone open-source [**Retrack**](https://github.com/secutils-dev/retrack) project (a git submodule at `components/retrack` in the Secutils.dev mono-repo) and runs as the **Retrack Web Scraper** on port `7272`.
- The "Resources Tracker" feature is now the unified [**Page tracker**](https://secutils.dev/docs/guides/web_scraping/page), there is also a separate [**API tracker**](https://secutils.dev/docs/guides/web_scraping/api) for HTTP API responses. Page trackers can also use the **Camoufox** stealth browser engine in addition to Chromium.
- The IP-validation logic is enforced in two places: in the Rust validator before a tracker is scheduled, and in Kubernetes `NetworkPolicy` rules at the egress layer.

The principles below have not changed, the references and class names have.
:::

For a deeper dive into the scraping mechanics themselves, see [**"How to track anything on the internet, or use Playwright for fun and profit"**](/blog/web-page-content-trackers-and-playwright).

## The threat model in one paragraph

The Retrack Web Scraper drives a real Chromium (or Camoufox) browser at user-supplied URLs. That makes it a powerful tool, and it makes it a great target. The two failure modes that matter most are **server-side request forgery** ("user points the browser at our internal network") and **resource exhaustion** ("user points the browser at a 5 GB MP4"). Everything below is about closing or shrinking those classes of failure.

## 1. Input validation

The first line of defence is to limit where users can point the browser. At a minimum:

- Allow only `http://` and `https://` URL schemes. Reject `file://`, `chrome://`, `devtools://`, `about:`, `view-source:`, `data:`, and friends. They unlock things you don't want unlocked.
- Resolve the hostname and reject any address that is not globally routable (loopback, link-local, private RFC 1918, IPv6 ULA, IPv4-mapped, documentation, multicast).
- Run validation **server-side**. Client-side validation is a UX nicety, never a security control; an attacker can hit your API directly.

A simplified version of the Rust validator looks like this:

```rust
if tracker.url.scheme() != "http" && tracker.url.scheme() != "https" {
    anyhow::bail!("Tracker URL scheme must be either http or https");
}

let is_public_host_name = if let Some(domain) = tracker.url.domain() {
    // Resolve and check that every resolved IP is globally routable.
    // ...
} else {
    false
};

if !is_public_host_name {
    anyhow::bail!("Tracker URL must have a valid public reachable domain name");
}
```

The full IP-validation logic is in [**Part 3 of the "Detecting changes in JS/CSS"**](/blog/detecting-changes-in-js-css-part-3#challenge-8-malicious-users) series. It runs both at scheduling time **and** at fetch time, because DNS rebinding can move a name's resolution between the two.

## 2. Resource isolation

A headless browser is heavy even when it's quiet. The component that owns it has no business sharing a process (or even a node) with the parts of your service that handle authentication, billing, or anything else critical. So:

- Put the scraper in its **own container** (in Secutils.dev that's the Retrack Web Scraper). Scale it independently of the API.
- Apply explicit **resource limits**. Both `requests` and `limits` for CPU and memory; otherwise a single hostile page can drive the node into swap.

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: retrack-web-scraper
spec:
  containers:
  - name: app
    image: secutils/retrack-web-scraper:latest
    resources:
      requests:
        memory: "128Mi"
        cpu: "250m"
      limits:
        memory: "1Gi"
        cpu: "500m"
```

Add hard timeouts at every level: the HTTP fetch, the page render, the user script execution. A single missing timeout is enough to wedge a worker.

## 3. Privilege management

[**Principle of least privilege**](https://en.wikipedia.org/wiki/Principle_of_least_privilege) applies extra hard to a process that drives a web browser:

- **Run as a non-root user**. Both `node` and `chromium` should run unprivileged.

  ```dockerfile
  FROM node:22-alpine
  # ...
  USER node
  CMD [ "node", "src/index.js" ]
  ```

- In Kubernetes, **drop all capabilities** for the scraper container:

  ```yaml
  securityContext:
    capabilities:
      drop: [ ALL ]
    runAsNonRoot: true
    allowPrivilegeEscalation: false
  ```

- Apply a **seccomp profile** that allows only the syscalls the browser actually needs. The Playwright maintainers ship a known-good profile that is a great starting point:

  ```yaml
  securityContext:
    seccompProfile:
      type: Localhost
      # Based on https://github.com/microsoft/playwright/tree/main/utils/docker
      localhostProfile: retrack-web-scraper-seccomp-profile.json
  ```

## 4. Browser sandbox

Once you're running as non-root, **enable the Chromium sandbox**. People disable it because the error messages are confusing; the price of doing so is enormous. From [**no-sandbox.io**](https://no-sandbox.io/): with the sandbox off, a single browser bug becomes a full-process compromise.

```javascript
import { chromium } from 'playwright';

const browser = await chromium.launch({
  chromiumSandbox: true,
});
```

Camoufox (Secutils.dev's stealth engine for sites that fingerprint Chromium) takes the same option. Don't ship a scraper without one of them on.

## 5. Network policies

Even with watertight input validation, software has bugs. Defence in depth means assuming the validator will eventually slip and putting a second control at the network layer. Kubernetes [**`NetworkPolicy`**](https://kubernetes.io/docs/concepts/services-networking/network-policies/) gives you a clean place to refuse egress to private IP ranges:

```yaml
kind: NetworkPolicy
apiVersion: networking.k8s.io/v1
metadata:
  name: retrack-web-scraper-network-policy
  namespace: secutils
spec:
  policyTypes: [ Egress ]
  podSelector:
    matchLabels:
      app: retrack-web-scraper
  egress:
  - to:
    - ipBlock:
        cidr: 0.0.0.0/0
        except:
          - 10.0.0.0/8
          - 172.16.0.0/12
          - 192.168.0.0/16
          - 169.254.0.0/16  # link-local + cloud metadata
```

Add explicit egress rules for the Retrack API and any caches/CDNs the scraper should reach in the cluster. Everything else (database, secrets manager, internal admin services) should be unreachable.

## 6. Monitoring

A scraper that's been running quietly for six months is the one you should distrust. Threats evolve, dependencies change, configurations drift. The mitigations above only stay effective if you're watching:

- **Resource utilisation** with alerts on unexpected spikes (memory, CPU, network egress, container restarts).
- **Brute-force / DDoS** detection on the Retrack and API endpoints.
- **Unexpected errors** and **service crashes**, especially anything browser-internal that suggests a sandbox escape attempt.

Secutils.dev uses the self-hosted Elastic Stack covered in [**"Privacy-friendly usage analytics and monitoring"**](/blog/usage-analytics-and-monitoring), but the same shape (logs + metrics + alerts) works with any stack: Datadog, Grafana + Loki + Prometheus, or even plain `journalctl` + an alerting cron.

## Frequently asked questions

### Is the scraper open-source?

Yes. The whole engine is the [**Retrack**](https://github.com/secutils-dev/retrack) project, included in the Secutils.dev mono-repo as the `components/retrack` git submodule.

### Why two layers of IP validation (Rust + NetworkPolicy)?

Because they fail differently. The Rust validator catches obviously bad URLs cheaply, before any work is dispatched. The `NetworkPolicy` catches whatever the validator missed (bugs, future regressions, DNS rebinding, IPv6 corner cases). Together they make a much narrower attack surface than either alone.

### What about cloud metadata endpoints (`169.254.169.254`, `100.100.100.200`)?

Both layers reject link-local addresses, including the major cloud metadata IPs. The `NetworkPolicy` example above explicitly excludes `169.254.0.0/16`.

### Can I run user-supplied JavaScript safely in this model?

Yes, that is exactly what the Page tracker [**extractor scripts**](https://secutils.dev/docs/guides/web_scraping/page) do. They run inside the page context (so they're constrained by the same-origin policy and the Chromium sandbox), with strict execution-time and memory limits enforced by the embedded Deno runtime on the Secutils.dev side. See [**"Building a Rust application with embedded JavaScript extensions"**](/blog/rust-application-with-js-extensions) for the runtime details.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
