---
title: Detecting changes in JavaScript and CSS isn't an easy task, Part 3
description: "Part 3: security hardening for a web page resource tracker. Capturing onload/onerror payloads, supporting authenticated pages with custom HTTP headers and Playwright session simulation, and protecting the scraper from malicious users with URL-scheme allow-lists, IP filtering, and timeouts."
slug: detecting-changes-in-js-css-part-3
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-07-18_web_page_resources_tracker.png
tags: [thoughts, overview, technology]
keywords: [web scraper security hardening, ssrf prevention, ip range allowlist, onload onerror tracking, authenticated web scraping, malicious users, page tracker security, secutils.dev, retrack]
---

Hello!

This is the third and final post in a series ([**Part 1**](/blog/detecting-changes-in-js-css-part-1), [**Part 2**](/blog/detecting-changes-in-js-css-part-2)) on the surprisingly hard problem of detecting changes in a web page's JavaScript and CSS resources, written while building the Resources Tracker (now [**Page tracker**](https://secutils.dev/docs/guides/web_scraping/page)) feature in [**Secutils.dev**](https://secutils.dev).

The previous posts covered scraping mechanics and storage. Today we look at the security side: what extra parts of the page have to be tracked to catch tampering, what it takes to scrape authenticated pages, and what defences a tool like this needs against malicious users (since "scrape an arbitrary URL" is a powerful primitive).

<!--truncate-->

:::info UPDATE (May 2026)
Two updates relevant to this post:

- **Authenticated scraping** is supported today. [**Page trackers**](https://secutils.dev/docs/guides/web_scraping/page) accept custom HTTP headers, can use **user secrets** for credentials (so they never appear in plaintext), and tracker extractor scripts can be imported directly from Playwright codegen output to capture full login flows. Stealth-grade scraping is also available via the **Camoufox** browser engine.
- **Operational hardening** of the scraper has matured significantly. The dedicated [**"Running web scraping service securely"**](/blog/running-web-scraping-service-securely) post covers the current end-to-end picture (resource isolation, non-root + Chromium sandbox, seccomp, network policies). The IP-validation snippet below still reflects the spirit of the current implementation.
:::

## Challenge 6: HTML `onload` and `onerror` attributes

Tracking the URL and content of every `<script>` and `<link rel="stylesheet">` is necessary, but not sufficient. The `<script>` and `<link>` elements also support [**`onload` and `onerror`**](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/load_event) attributes, which contain inline JavaScript executed when the resource loads (or fails to). A subtle attacker could leave the `src` URL pointing at a perfectly legitimate library while sneaking malicious behaviour into `onload`:

```html
<script src="https://some-legit-url" onload="alert('😈')"></script>
```

The fix is to fold these attributes into the resource fingerprint. In Secutils.dev the contents of `onload` and `onerror` are concatenated with the resource body before computing the locality-sensitive hash, so any change to the inline handler shows up as a change to the resource's fingerprint.

## Challenge 7: Authenticated (protected) pages

The whole series so far has implicitly assumed that the target page is reachable by an unauthenticated client. Plenty of pages aren't. A real change-tracker has to support a few common authentication patterns.

The simplest is HTTP Basic / Bearer authentication: the user provides an `Authorization` header, and the scraper attaches it to every request. Secutils.dev's [**Page trackers**](https://secutils.dev/docs/guides/web_scraping/page) accept arbitrary custom headers for exactly this reason, including `Authorization` and `Cookie`.

For credentials that you don't want stored in plaintext (and you really shouldn't), Secutils.dev supports [**user secrets**](https://secutils.dev/docs/guides/platform/secrets): encrypted-at-rest values that you reference by name from a tracker or script. The actual secret value never appears in the tracker definition, the request log, or the Web UI.

For sites that don't accept long-lived sessions (e.g. multi-step OAuth flows), the approach is different: capture the full login flow once with **Playwright codegen** and import the recorded script as a tracker [**extractor script**](https://secutils.dev/docs/guides/web_scraping/page). The scraper then re-runs the recorded login before extracting resources for each scheduled check.

## Challenge 8: Malicious users

Building a tool with a public scraping primitive means accepting that some users will try to abuse it. "Run a browser at this URL" is enough rope to do a lot of damage if the service trusts the input.

The mental model that has held up best for me is: **start as restricted as possible, then carefully relax restrictions where the value is clearly worth the risk**.

### Restrict URL schemes

Decide which URL schemes you'll accept. Fewer is safer. Secutils.dev allows only `http` and `https`. Schemes like `file://` (local filesystem), `chrome://` / `devtools://` (browser internals), `about:`, `view-source:`, and so on are all rejected at validation time. This kills off a long list of "creative" attacks.

### Block private and special-use IP ranges

Even a strict scheme allow-list isn't enough if a hostname resolves to your own internal network. SSRF (Server-Side Request Forgery) is the classic threat: a malicious user submits a URL that resolves to `169.254.169.254` (cloud metadata) or `10.0.0.5` (your internal database), and the scraper happily fetches it.

Secutils.dev validates that the resolved address is **globally routable** before fetching. The reduced shape of the check looks like this (Rust):

```rust
impl IpAddrExt for IpAddr {
    fn is_global(&self) -> bool {
        if self.is_unspecified() || self.is_loopback() {
            return false;
        }

        match self {
            IpAddr::V4(ip) => {
                // "This network", private, broadcast, link-local, multicast, etc.
                !(ip.octets()[0] == 0
                    || ip.is_private()
                    || ip.is_broadcast()
                    || /* ... */)
            }
            IpAddr::V6(ip) => {
                // IPv4-mapped, documentation, ULA, multicast, etc.
                !(matches!(ip.segments(), [0, 0, 0, 0, 0, 0xffff, _, _])
                    || (ip.segments()[0] == 0x2001) && (ip.segments()[1] == 0xdb8)
                    || /* ... */)
            }
        }
    }
}
```

This is enforced both at validation time (before scheduling the scrape) and at fetch time (DNS rebinding can move a name from a public IP to a private one between checks). Belt and braces.

### Defence in depth at the infrastructure level

Application-level validation is necessary but not sufficient. The Retrack scraper container also has Kubernetes `NetworkPolicy` rules that block egress to the standard private IPv4 ranges (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`). If a bug ever lets a user-supplied URL slip past the Rust validator, the network layer still refuses to forward the request.

The [**dedicated security post**](/blog/running-web-scraping-service-securely) covers the rest of the deployment-level hardening (non-root user, Chromium sandbox, seccomp profile, resource limits).

### Fail safely

A few smaller habits that pay off:

- **Don't return raw error messages.** Log the original on the server, return something generic to the user. Otherwise SSRF probes turn into a useful "what does this internal hostname respond with?" oracle.
- **Hard timeouts** on every fetch, every script, and every page render. Otherwise a single slow target can pin a worker indefinitely.
- **Rate limits and per-tracker size limits** on captured payloads. Web pages can be enormous, and an attacker would happily point your scraper at a multi-gigabyte response.

These are the obvious starting points. Production-grade hardening goes well beyond this list, but covering the basics in code blocks an enormous fraction of attempted abuse.

## Wrap-up

Tracking changes in JavaScript and CSS resources is doable but fiddly. Inline vs external resources, dynamic loading, large content, `data:` and `blob:` URLs, noisy inline scripts, `onload`/`onerror` payloads, authenticated pages, and abusive users are all problems you'll meet sooner or later. None are individually hard; what makes a real-world tracker hard is having to handle all of them at once without the noise crowding out actual change signals.

If you'd rather skip the implementation work, you can use [**Page trackers**](https://secutils.dev/docs/guides/web_scraping/page) and [**API trackers**](https://secutils.dev/docs/guides/web_scraping/api) in Secutils.dev directly, the scheduling/scraping engine ([**Retrack**](https://github.com/secutils-dev/retrack)) is open-source.

## Frequently asked questions

### Does Secutils.dev support scraping authenticated pages today?

Yes. Custom HTTP headers, [**user secrets**](https://secutils.dev/docs/guides/platform/secrets) for credential storage, and Playwright codegen import for full login flows are all supported on the [**Page tracker**](https://secutils.dev/docs/guides/web_scraping/page).

### What stops users from scraping internal infrastructure?

Layered defences: URL scheme allow-list, application-level "is this IP globally routable" validation at both schedule time and fetch time, plus Kubernetes `NetworkPolicy` rules that block egress to private IP ranges from the scraper container.

### What about cloud metadata endpoints (`169.254.169.254`)?

The IP-validation step rejects link-local addresses, including the IMDS endpoint, before the request is dispatched. The network policy rejects them again at the egress layer.

### Is the scraper open-source?

Yes. The whole scraping engine is the [**Retrack**](https://github.com/secutils-dev/retrack) project, included in the Secutils.dev mono-repo as the `components/retrack` git submodule.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
