# SEO head and OG images

Read before adding a tool or changing its `<head>`, `<noscript>`, related-tools banner, or OG image. `assertSeoBasics` in `e2e/tools/_helpers.ts` enforces the head block.

## SEO requirements

The free tools double as a lead magnet: every page lives at a stable URL on
`{{TOOLS_HOST}}` and is the first impression a search-result or LLM-citation
visitor gets. Each tool HTML must therefore ship the full SEO head block below
(use `jwt-debugger.html` as the canonical reference). Per-tool E2E specs in
`e2e/tools/<slug>.spec.ts` enforce these via `assertSeoBasics`, so a missing
or empty tag fails CI.

### Required `<head>` tags

```html
<title>{{Tool}}: {{Snappy Subtitle}} | Secutils.dev</title>
<meta name="description"     content="{{60-160 chars; mention what it does, who it's for, and 'no signup'.}}">
<meta name="robots"          content="index, follow, max-image-preview:large">
<link rel="canonical"        href="https://{{TOOLS_HOST}}{{su-tool-path}}">  <!-- own-host app: https://{{MARS_HOST}}/ -->

<!-- Introspection (read by tools-check.ts and the agent surface) -->
<meta name="su-tool-path"        content="{{path}}">
<meta name="su-tool-name"        content="{{Tool Name}}">
<meta name="su-tool-description" content="{{One-line marketing description}}">
<meta name="su-tool-promote"     content="true|false">

<!-- Open Graph (rich previews on Slack, GitHub, LinkedIn, …) -->
<meta property="og:type"        content="website">
<meta property="og:site_name"   content="Secutils.dev">
<meta property="og:title"       content="{{Tool}}: {{Snappy Subtitle}}">
<meta property="og:description" content="{{Same as meta description, may shorten.}}">
<meta property="og:url"         content="https://{{TOOLS_HOST}}{{su-tool-path}}">
<meta property="og:image"       content="https://secutils.dev/docs/img/og/og-{{slug}}.png">
<meta property="og:image:width"  content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt"   content="{{Same as title, used by screen readers.}}">
<meta property="og:locale"      content="en_US">

<!-- Twitter card (single image, no light/dark variant) -->
<meta name="twitter:card"        content="summary_large_image">
<meta name="twitter:title"       content="{{Tool}}: {{Snappy Subtitle}}">
<meta name="twitter:description" content="{{Same as og:description.}}">
<meta name="twitter:image"       content="https://secutils.dev/docs/img/og/og-{{slug}}.png">

<!-- JSON-LD: WebApplication for tools, ItemList for index.html -->
<script type="application/ld+json">{
  "@context": "https://schema.org",
  "@type": "WebApplication",
  "name": "{{Tool Name}}",
  "url": "https://{{TOOLS_HOST}}{{su-tool-path}}",
  "applicationCategory": "SecurityApplication",
  "operatingSystem": "Any",
  "browserRequirements": "Requires JavaScript",
  "isAccessibleForFree": true,
  "offers": { "@type": "Offer", "price": "0", "priceCurrency": "USD" },
  "publisher": { "@type": "Organization", "name": "Secutils.dev", "url": "https://secutils.dev" },
  "sameAs": "https://github.com/secutils-dev/secutils/blob/main/dev/tools/{{file}}.html",
  "description": "{{Longer paragraph for SEO, can repeat the meta description.}}"
}</script>
```

### Required body elements

- A visible **`<noscript>` paragraph** at the top of `<body>` that explains the
  tool needs JavaScript and links back to the secutils.dev home. SEO crawlers
  treat this as the page's text content when JS is disabled, so it must be
  meaningful (not just "JS required").
- A **bottom "more free tools" banner** as the last child of `<main>` (see
  [chrome.md](chrome.md#more-free-tools-bottom-cta)). This is the **only** related-tools surface
  on the page - it carries both the SEO internal-linking value (one link from
  every leaf back to the index) and the human / agent discovery affordance.

  `mock-saml-idp.html` (unpromoted) still carries the retired `<nav class="su-related">` list instead of the banner.
  When editing it for another reason, replace that block and its `.su-related*` CSS with the banner.

### Per-tool OG image

Every tool ships a 1200x630 OG image at
`https://secutils.dev/docs/img/og/og-<slug>.png` (and a sibling
`og-<slug>-light.png` for light-themed previews). These are auto-generated; do
not paint them by hand. See **OG image generation** below.

## OG image generation

OG images are rendered at deploy time by the existing Playwright stack. Source
template: [`dev/tools/og-template.html`](../og-template.html), parameterised via
URL query strings (`name`, `path`, `desc`, `accent`, `icon`, `theme`, `host`).
The driver spec [`e2e/tools/og.spec.ts`](../../../e2e/tools/og.spec.ts) iterates
over [`e2e/tools/registry.ts`](../../../e2e/tools/registry.ts) and writes both a
dark and a light PNG per tool into
`components/secutils-docs/static/img/og/`. Docusaurus serves `static/*`
verbatim, so the final URLs are stable and unhashed.

```bash
# Regenerate every OG image (a dark + light pair per registry.ts entry, index included)
make tools-og

# Verify byte-stability (re-runs N times, checks the files do not change)
make tools-og-loop RUNS=5
```

An own-host app (`host` in `registry.ts`, so MARS) uses `layout=app`: its logo and name are centered, because Slack
and other unfurlers crop the card to a square thumbnail. It also gets an `og-<slug>.jpg`, which `deploy.ts` embeds in
its responder script (`{{OG_JPEG_BASE64}}`), so the image is served at `https://<host>/og.jpg` and updates with a tools
deploy instead of a docs deploy. Unfurlers cache previews per URL, so a changed image may need a new URL.

The same stability guarantees as the docs screenshot suite apply: pre-screenshot
DOM stabilization in `goto()`, sticky-pixel re-encoding to absorb sub-pixel
anti-aliasing jitter, fixed viewport at exactly 1200x630 so no scaling
math is involved. Adding a new tool is a one-row diff in `registry.ts` plus a
`make tools-og` to materialise the PNGs.
