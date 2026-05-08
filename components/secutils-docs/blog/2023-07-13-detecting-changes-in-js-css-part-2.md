---
title: Detecting changes in JavaScript and CSS isn't an easy task, Part 2
description: "Part 2: handling data: and blob: URLs as 'inline' resources, and using TLSH fuzzy hashing (Locality Sensitive Hashing) to track changes in noisy inline scripts in Secutils.dev's Page tracker."
slug: detecting-changes-in-js-css-part-2
authors: azasypkin
image: https://secutils.dev/docs/img/blog/2023-07-13_web_page_resources.png
tags: [thoughts, overview, technology]
keywords: [data url javascript, blob url tracking, fuzzy hashing, tlsh, locality sensitive hashing, malware detection technique, page tracker, secutils.dev, retrack]
---

Hello!

This is Part 2 of [**a three-part series**](/blog/detecting-changes-in-js-css-part-1) on the surprisingly hard problem of detecting changes in a web page's JavaScript and CSS resources, written while building the Resources Tracker (now [**Page tracker**](https://secutils.dev/docs/guides/web_scraping/page)) feature in [**Secutils.dev**](https://secutils.dev).

In Part 1 we covered inline vs external resources, dynamically loaded resources, and how to keep storage costs low with hashing. Today we tackle two more challenges: resources that don't fit cleanly into "inline" or "external", and inline resources that change on every page load even though "nothing meaningful" changed.

<!--truncate-->

:::info UPDATE (May 2026)
Same context as in Part 1: the "Resources Tracker" described here ships today as the unified [**Page tracker**](https://secutils.dev/docs/guides/web_scraping/page), and the underlying Playwright + TLSH pipeline now lives in the [**Retrack**](https://github.com/secutils-dev/retrack) submodule.
:::

## Challenge 4: Data and blob URLs

Telling inline and external resources apart usually comes down to whether `<script>` has a `src` attribute, and whether `<link rel="stylesheet">` has an `href`. Part 1's approach was: parse inline resources directly, intercept network requests for external resources via Playwright.

But not every "external-looking" resource is genuinely external. Two URL schemes break the assumption:

- **`data:` URLs** (e.g. `data:application/javascript;base64,YWxlcnQoMSk=`) embed the content directly in the URL itself.
- **`blob:` URLs** (e.g. `blob:aac12324xxxxxx`) point at an in-memory blob created by the page.

Neither triggers a network request that the scraper can intercept. Effectively they are inline resources wearing an external-looking URL. To capture them we have to handle them in-page:

```ts
// Split the data URL to extract the base64-encoded content.
const [/* data URL header */, dataUrlContent] = dataUrl.split(',');

// Fetch the blob URL via the Fetch API to retrieve the content.
const blobUrlContent = await fetch(url).then((res) => res.text());
```

Once parsed/fetched, we can run them through the same fingerprinting pipeline as inline resources.

## Challenge 5: Constantly changing inline resources

Suppose we crack everything above. We still have one painful case left: telling **changed** inline resources apart from **newly added** ones.

External resources have a stable identifier (the URL). Inline resources don't, so the only thing we can hang an identity off is the content digest. The problem is that digests are deliberately fragile: a one-byte change yields a completely different SHA-1 hash.

```html
<!-- Inline script #1 -->
<script>alert(1)</script>
<!-- SHA1 of the content -->
739033c41a7b1047bb6a63240cbe240cd06597cd

<!-- Same script, slightly changed -->
<script>alert(2)</script>
<!-- SHA1 of the changed content (completely different!) -->
c3d954707f1c1043158a4ed38f52776e7859e80c
```

A naive change-detector sees one resource removed and a different one added. That's noisy.

The problem is much worse in the wild. Many pages embed analytics or feature-flag scripts that bake a per-user random ID into the script body (or include cache-busting timestamps). Every page load yields a "different" inline resource, drowning real changes in noise.

### Fuzzy hashing to the rescue

The trick is to borrow a technique from **malware detection**: [**fuzzy hashing**](https://en.wikipedia.org/wiki/Fuzzy_hashing). Unlike cryptographic hashes, a fuzzy hash is designed so that **similar inputs produce similar hashes**, with a quantifiable distance metric between them. Malware authors use small mutations to evade signature-based detection, defenders use fuzzy hashes to spot the family despite the mutations.

Two well-known approaches are Context Triggered Piecewise Hashing (CTPH, e.g. ssdeep) and Locality Sensitive Hashing (LSH). Secutils.dev uses [**Trend Micro Locality Sensitive Hashing (TLSH)**](https://tlsh.org/), which has very mild input requirements (~50 bytes with sufficient randomness). That covers nearly every real inline web resource.

A quick demonstration:

```bash
$ cat inline-revision-0.js
alert(1);
alert(2);
alert(3);
alert(4);
alert(5);
alert(6);
alert(7);

$ cat inline-revision-1.js
alert(1);
alert(2);
alert(3);
alert(400); # changed
alert(500); # changed
alert(6);
alert(7);

$ tlsh -f inline-revision-0.js
T1C4A0025D65B74CD0C3B69F48020CD01304000118314F0D42000F81DC1019342C001404

$ tlsh -f inline-revision-1.js
T1B9A0024D65730CC0D77A9F48012CD00746000018318F0D42000F80DC1019342E003404

$ tlsh -c inline-revision-0.js -f inline-revision-1.js
25  # small distance: the files are clearly very similar
```

For each inline resource Secutils.dev computes both a SHA-1 (for exact matching) and a TLSH (for similarity matching). When two revisions are diffed, resources with identical SHA-1 are exact matches; those without an exact match are then compared by TLSH distance to identify "the same resource, slightly changed". Falls back to "small enough to store" or "no randomness" cases happen rarely in practice.

Fuzzy hashing turns a previously hopeless problem ("which of these unidentified inline scripts are the new versions of which?") into a tractable one, and keeps the storage cost per revision tiny.

## Where this is heading

Two challenges down, four to go. [**Part 3**](/blog/detecting-changes-in-js-css-part-3) tackles the security-flavoured ones: capturing `onload`/`onerror` payloads, supporting authenticated pages, and hardening the scraper against malicious users.

## Frequently asked questions

### Why TLSH instead of ssdeep?

Both work. TLSH was a slightly better fit because of its low minimum input size, the published distance metric, and a well-maintained Rust binding. For this use case the differences are minor.

### Could you skip fuzzy hashing entirely if you stored full content?

Yes, but then you'd be paying full storage cost per revision per resource, and you'd still need a heuristic to identify "this is the same script, just mutated". Fuzzy hashing solves both problems in one cheap step.

### Are `data:` and `blob:` URLs common in real pages?

`data:` URLs show up routinely for small inline images, fonts, and occasionally tiny scripts. `blob:` URLs are common in apps that use the Web Workers API, dynamic ESM imports, or libraries that synthesise scripts at runtime. Treating both as effectively-inline made tracker results much more accurate.

That wraps up today's post, thanks for taking the time to read it!

:::info ASK
If you found this post helpful or interesting, please consider showing your support by starring [**secutils-dev/secutils**](https://github.com/secutils-dev/secutils) GitHub repository. Also, feel free to follow me on [**Twitter**](https://twitter.com/aleh_zasypkin), [**Mastodon**](https://infosec.exchange/@azasypkin), [**LinkedIn**](https://www.linkedin.com/in/azasypkin/), [**Indie Hackers**](https://www.indiehackers.com/azasypkin/history), or [**Dev Community**](https://dev.to/azasypkin).

Thank you for being a part of the community!
:::
