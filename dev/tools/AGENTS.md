# Secutils.dev Single-Page Tool Apps - Style Guide

All tools in `dev/tools/` are standalone single-HTML-file apps (embedded CSS + JS) styled to look consistent with the Secutils.dev web application. Use `md.html` as the canonical reference implementation.

## Browser-support policy

Target **evergreen browsers** (current Chrome / Firefox / Safari / Edge). Allow **Baseline
Newly Available** features when they meaningfully simplify a tool (replace ad-hoc
JavaScript with native browser behaviour, drop a transitive CSS-positioning library, etc.).

When a Newly Baseline feature is not yet supported in one evergreen engine, a custom
fallback is allowed only if **all** of the following hold:

- The fallback adds **≤20 lines** of inline code (HTML, CSS, or JS combined).
- It does **not** require an external dependency, polyfill bundle, or CDN script.
- The feature degrades gracefully — the tool stays usable when the fallback is absent.

When a Newly Baseline feature does not meet the bar above (e.g. fallback would be more
than 20 lines or would need a polyfill), **skip the feature** and stay on the existing
pattern. Never load a polyfill bundle (no `invokers-polyfill`, no `@oddbird/popover-polyfill`,
no `scroll-timeline-polyfill`); the tools are intentionally dependency-free.

**Limited Availability** features (anything still missing in one of Chrome / Firefox /
Safari) are out of scope. Scroll-driven animations, view transitions, and similar
showcase-only APIs may be reconsidered when they reach Newly Baseline status.

This policy is consumed by the Modern Web Guidance skill at install time
(<https://developer.chrome.com/docs/modern-web-guidance>) and by any agent that asks
"should I add a fallback for this Baseline feature?" — the answer is the four bullets
above.

## Format

- Single `.html` file, no external dependencies except CDN-hosted libraries (fonts, highlight.js, etc.)
- Title format: `<title>Tool Name | Secutils.dev</title>`
- Default to `data-theme="dark"` on `<html>`, with theme toggle

## Responder Path Alias (`su-tool-path`)

Every tool HTML file (including `index.html`) must contain a `<meta name="su-tool-path">`
tag in `<head>` that declares the URL path where the tool is hosted as a responder. The
filename on disk does **not** need to match the responder path - the meta tag is the
source of truth.

```html
<meta name="su-tool-path" content="/jwt">
```

The current file ↔ path mapping lives in [`e2e/tools/registry.ts`](../../e2e/tools/registry.ts)
(it imports the HTML files at parse time and re-exports the meta-tag values). Look
there if you need a quick listing - do not maintain a parallel table here that can drift.

When **creating a new tool**, **deleting a tool**, or **changing a tool's alias**:

1. **Set or update the `su-tool-path` meta tag** in the tool's own HTML file. This is
   always the first step - the meta tag is the canonical reference for the responder path.

2. **Update `index.html`** - the tool index page at `dev/tools/index.html` must stay in
   sync. Each tool is a `<a class="tool-card">` entry in the `.tool-list` container. To
   keep it consistent:

   - **Adding a tool**: read the new file's `su-tool-path` value and add a new card entry
     with the correct `href`, path badge, tool name, and description.
   - **Removing a tool**: delete the corresponding `<a class="tool-card">` block.
   - **Changing an alias**: update the `href` attribute and the `<span class="tool-path">`
     text in the existing card to match the new `su-tool-path` value.

   Card entry format:

   ```html
   <a class="tool-card" href="/the-path">
       <div class="tool-name">Tool Name <span class="tool-path">/the-path</span> <span class="arrow">&rarr;</span></div>
       <div class="tool-desc">Short description of what the tool does.</div>
   </a>
   ```

3. **Update the table above** in this AGENTS.md file to keep the mapping accurate.

4. **Update [`e2e/tools/registry.ts`](../../e2e/tools/registry.ts)** - append a row
   with the tool's slug, source filename, accent color, and OG icon symbol. The
   registry holds **OG-generation and E2E-specific metadata** that does not belong
   in user-facing HTML (accent color, icon glyph, application category). It does
   **not** restate the tool's name / path / description / promotion - those live in
   the HTML `<meta>` tags (the registry just imports them at parse time so there is
   no double-source).

   Source-of-truth split:

   | Field                                               | Lives in                | Read by                                                                           |
   |-----------------------------------------------------|-------------------------|-----------------------------------------------------------------------------------|
   | `su-tool-name`, `-path`, `-description`, `-promote` | tool HTML `<meta>` tags | deploy.ts (llms.txt, sitemap, agent-skills/index), tools-check.ts, marketing site |
   | accent colour, OG icon, application category        | `e2e/tools/registry.ts` | `og.spec.ts` OG generator, per-tool E2E specs                                     |

## Promotion (`su-tool-promote`)

Every tool also carries a `<meta name="su-tool-promote" content="true|false">` tag in
`<head>` that decides whether the tool is publicly discoverable at all (beyond a
direct link an operator shares out of band):

- `true` - listed everywhere: on `dev/tools/index.html`, on the marketing site's home
  page in the "Free tools, no signup required" card section (anchored at
  `#free-tools`), in the root `README.md` table, and in every agent-discovery
  aggregate (`llms.txt`,
  `sitemap.xml`, `.well-known/agent-skills/index.json`). This is the default for any
  user-facing free tool.
- `false` - listed **nowhere**: not on `dev/tools/index.html`, not on the marketing
  site, not in `README.md`, not in `llms.txt` / `sitemap.xml` /
  `.well-known/agent-skills/index.json`, and the page itself carries
  `<meta name="robots" content="noindex, nofollow">` so search engines drop it from
  their corpus too. The HTML responder and its `<path>.md` SKILL companion are still
  deployed (and `Accept: text/markdown` content negotiation still works), so a direct
  link an operator shares out of band keeps working - it's just not advertised
  anywhere. Use this for niche tools, e.g. `mock-saml-idp` (Elasticsearch / Kibana
  SSO testing only).

`deploy.ts` enforces this filter in `buildLlmsTxt`, `buildSitemapXml`, and
`buildAgentSkillsIndex` - non-promoted tools are filtered out, not reordered. The
per-tool `*.skill.md` deploy walk is independent of those aggregates, so flipping
`promote` to `false` does not break the direct `<path>.md` URL.

`make tools-check` (Node script at `scripts/tools-check.ts`, run directly via
Node 24+ type stripping) walks every `dev/tools/*.html`, reads its `su-tool-promote`
value, and asserts that the marketing home page and `README.md` link only to
`promote=true` tools. The marketing site lives in a separate (private) sibling
checkout, so its location is supplied via the `SECUTILS_TOOLS_PROMO_HOME_INDEX`
env var (absolute path to its `index.html`, or a path relative to this repo
root). When the env var is unset the marketing-side check is skipped with a
warning; the README, skill-sibling, and non-promoted-leak checks still run.
The e2e suite (`e2e/tools/index.spec.ts` and `e2e/tools/registry.spec.ts`)
covers the inverse - non-promoted tools must be absent from the index page and
every aggregate, while their `.md` companion must still be reachable. Run it
after touching either side of that boundary; CI runs it on every push.

## Host config and templating (`{{TOOLS_HOST}}`)

Every tool is served from a single, configurable subdomain - defaults to
`tools.secutils.dev` - controlled by one environment variable:

```bash
# .env (root)
SECUTILS_TOOLS_PUBLIC_HOST=tools.secutils.dev
```

Both repos respect this variable so a single rename rolls through the whole stack:

- **`dev/tools/deploy.ts`** substitutes `{{TOOLS_HOST}}` in every `.html` and
  `.skill.md` source **before** minification, so the deployed responder body /
  markdown contains the real host. Affected places: `<title>` text, `<link
  rel="canonical">`, `og:url`, `og:image` (if local), JSON-LD `"url"`, the
  Related-tools navigation block, and every `wire_format.url` in the `.skill.md`
  frontmatter.
- **The marketing site** (Parcel build, sibling repo) consumes the same variable
  via two parallel mechanisms - `posthtml-expressions` exposes it as a local for
  HTML files (templates `{{ TOOLS_HOST }}`), and a tiny custom Parcel transformer
  substitutes it in `sitemap.xml` and any other non-HTML asset.

When authoring a new tool **always reference the host via `{{TOOLS_HOST}}`** in the
sources - never hard-code `tools.secutils.dev`. The placeholder is also recognised
inside `*.skill.md` frontmatter and inside the body (so wire-format examples use the
configured host).

## Responder Script (`@su:responder-script`)

Most tools in `dev/tools/` are pure client-side HTML - the responder just serves a static
body. A few tools also need a small server-side script (e.g. `echo.html` decodes a `?c=…`
query parameter and returns a synthesised HTTP response). To keep the HTML the single
source of truth for both halves, embed the responder script in an HTML comment with the
`@su:responder-script` marker:

```html
<!DOCTYPE html>
<!-- @su:responder-script
// Optional human-readable preamble as JS // comments - these survive into the
// deployed responder script (and are stripped by the responder backend if it
// minifies; harmless either way).
(() => {
  const encoded = context.query.c;
  if (!encoded) return null;        // fall through to the static HTML body
  // ...handle the configured request...
})();
-->
<html lang="en">
…
</html>
```

How the deploy pipeline treats it:

1. [`dev/tools/deploy.ts`](deploy.ts) reads the file, finds the **first** comment whose
   first non-whitespace content is the marker `@su:responder-script`, and captures the
   trimmed body (everything between the marker line and `-->`).
2. The same `html-minifier-terser` invocation that builds the deployed body strips the
   comment via `removeComments: true`, so the script never reaches end users.
3. The PUT to `/api/webhooks/responders/{id}` includes both `settings.body` (minified
   HTML) and `settings.script` (extracted JS), so a single deploy keeps the two in sync.
4. The deploy log shows the script size next to the body size, e.g.
   `21.1 KB -> 16.2 KB (23.0% saved) + script 2.0 KB ✓ deployed`.
5. If the file has no marker comment, the deploy behaves as before - `script` is omitted
   from the PUT (and, since `ResponderSettings` is replaced wholesale, this clears any
   pre-existing script on the responder).

Rules and caveats:

- **Marker placement**: anywhere in the file, but immediately after `<!DOCTYPE html>` is
  the convention so it's easy to find.
- **Content is JavaScript**: everything after the marker line is treated as the script
  body, so any human-readable preamble must be written as `//` JS comments (not `=====`
  banners, which would be a syntax error in JS).
- **No `-->` inside the script**: the regex stops at the first `-->`. Vanishingly rare in
  JS - and would also break HTML parsing - but worth knowing.
- **Single match per file**: only the first marker comment is used; additional ones
  produce a yellow `⚠ multiple @su:responder-script comments found, using the first`
  warning in the deploy log.
- **Marker is opt-in**: most tools are static HTML and don't need this - leave it off and
  deploy ships the body alone.
- **Path type is NOT deployed - configure it on the responder.** `deploy.ts` PUTs only
  `settings` (body + script + headers); it never touches the responder's `location`, so the
  responder's `pathType` is whatever it was created with. Tools that route on a **sub-path**
  (e.g. `webhook.html` captures requests at `/webhook/<token>` while serving its configurator
  at the bare `/webhook`) **require a prefix responder** (`location.pathType: "^"`). If such a
  responder is created as an exact match (`pathType: "="`), the bare mount works but every
  sub-path returns an empty server-level `404` (no body, no `Content-Type`) because no
  responder matches - the request never reaches the script. Symptom to recognise: management
  calls (`/webhook?t=…`) succeed, but `/webhook/<token>` 404s with an empty body. Fix by
  switching the responder to prefix:
  ```bash
  curl -X PUT "$API/api/webhooks/responders/$RID" -H "Authorization: Bearer $KEY" \
    -H 'Content-Type: application/json' \
    -d '{"location":{"pathType":"^","path":"/webhook","subdomainPrefix":"tools"}}'
  ```
- **Composes with the auto-injected Markdown-negotiation prelude.** `deploy.ts` always
  wraps every HTML responder's script (whether opt-in or empty) with a ~250 B prelude
  that 302-redirects `Accept: text/markdown` requests to the `<slug>.md` sibling. Your
  `@su:responder-script` body becomes the inner expression and runs only when the
  prelude does not redirect. See **"Markdown content negotiation"** below.

## Embedded JS bundles (`data-su-bundle`)

Most tools are pure inline JS that fits comfortably inside the HTML. A few
need a real npm package that has Node-only dependencies (e.g. liteparse needs
`sharp` / `fs` / `child_process` stubbed out before it can run in the browser).
Pulling those into a tool means a bundler step. To keep that opt-in,
deterministic, and **bundled into the HTML responder body** (no separate
asset host, no extra requests), we use the `data-su-bundle` convention.

### Layout: one sub-package per bundle under `dev/tools/js/`

```
dev/tools/js/
  <name>/
    package.json          # own deps + scripts.build
    package-lock.json
    vite.config.ts        # (or rollup, esbuild, ...) - whatever the bundler is
    src/                  # source + stubs
    dist/<name>.js        # build output, gitignored
    .gitignore            # node_modules/, dist/
    README.md             # what this bundles and which upstream version is pinned
```

The single hard contract with the deploy pipeline is: `npm run build` inside
the sub-package must produce a single self-contained file at
`dist/<name>.js`. Everything else (which bundler, how it stubs Node modules,
whether it bundles workers inline, etc.) is the sub-package's business.

### HTML-side placeholder

The tool HTML references the bundle with an empty `<script>` placeholder:

```html
<script id="su-bundle-liteparse" type="text/plain"
        data-su-bundle="liteparse"></script>
```

Three load-bearing details:

- **`type="text/plain"`** keeps the browser from trying to execute the
  multi-MB ESM source on initial parse. The tool's own JS pulls the source
  out of `el.textContent`, wraps it in a Blob, and `import()`s the Blob URL
  on first use. Lazy by design - a search-result visitor pays the HTML
  download cost but never pays the JS parse/eval cost unless they actually
  click the tool's primary action.
- **`data-su-bundle="<name>"`** matches the sub-directory name under
  `dev/tools/js/`. Accepted character set: `[a-z0-9_-]+`.
- **The body must be empty.** `deploy.ts` refuses to overwrite a placeholder
  that already has content - the convention is opt-in, not opt-out, and a
  non-empty placeholder usually means someone forgot to clear test code.

### Canonical loader (copy verbatim into any tool that uses a bundle)

```js
async function loadSuBundle(name) {
    const el = document.getElementById(`su-bundle-${name}`);
    if (!el?.textContent) throw new Error(`Bundle "${name}" is not loaded`);
    const blob = new Blob([el.textContent], { type: 'text/javascript' });
    return import(URL.createObjectURL(blob));
}
```

The returned value is the module's namespace object, e.g.
`const { LiteParse } = await loadSuBundle('liteparse')`.

### How the deploy pipeline handles bundles

[`dev/tools/deploy.ts`](deploy.ts) does, for every HTML responder:

1. **After** `html-minifier-terser` runs (so the bundle source never passes
   through the HTML minifier - it's already minified by Vite and we don't
   want `collapseWhitespace` quirks corrupting an ESM module),
2. Scan the minified HTML for `<script ... data-su-bundle="<name>" ...></script>`
   placeholders.
3. For each unique `<name>`, ensure `dev/tools/js/<name>/dist/<name>.js` is
   fresh. Build rule: compare its mtime against the newest mtime under the
   sub-package (excluding `dist/` and `node_modules/`). If stale or missing,
   run `npm ci` (only when `node_modules/` is absent) then `npm run build`.
   Bundles are cached per `deploy.ts` invocation so multiple HTML files that
   share a bundle build it once.
4. Inject the bundle source as the placeholder's text content, escaping any
   `</script>` substring to `<\/script>` so the inlined script tag can't
   terminate early.
5. Log it alongside the body / script sizes, e.g.
   `21.1 KB -> 16.2 KB (23.0% saved) + bundle liteparse 1.8 MB ✓ deployed`.

Pre-building is optional. Run `make tools-bundles` once to warm every
sub-package's `dist/` (CI does this); after that, `make deploy-tools` is the
same fast path as for bundle-less tools.

### Rules and caveats

- **Sub-package isolation.** Each `dev/tools/js/<name>/` brings its own
  `node_modules/` and lockfile. Do not hoist to the repo-root `package.json`
  - the whole point is that bundles can ship Node-only deps without
  polluting the rest of the repo.
- **Pin upstream versions.** A bundle's `package.json` should pin its
  important deps (especially anything we monkey-patch via stubs / file
  redirects). Note the pin in the sub-package's `README.md` so a future
  re-sync against upstream has a clear starting point.
- **`text/plain` is final**, not `type="module"`. `type="module"` would
  execute on page load and force every visitor to pay the parse cost. The
  Blob+`import()` indirection costs one tick on first use and gains a clean
  no-cost-for-non-users default.
- **Bundle size matters but isn't policed.** Responders cap body size (and,
  separately, the PUT JSON payload). If the raw bundle pushes a responder
  over either cap, switch the placeholder to compressed-mode by adding
  `data-su-bundle-encoding="gzip-base64"`:

  ```html
  <script id="su-bundle-foo" type="text/plain"
          data-su-bundle="foo"
          data-su-bundle-encoding="gzip-base64"></script>
  ```

  `deploy.ts` then gzips the Vite/Rollup output, base64-encodes it (the
  base64 alphabet is `</script>`-safe so no further escaping is needed), and
  inlines that. The tool's runtime loader must reverse both steps before
  Blob-URL'ing the result:

  ```js
  const encoding = el.getAttribute('data-su-bundle-encoding');
  let src = el.textContent.trim();
  if (encoding === 'gzip-base64') {
      const bin = atob(src);
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      const stream = new Blob([bytes]).stream()
          .pipeThrough(new DecompressionStream('gzip'));
      src = await new Response(stream).text();
  }
  const blob = new Blob([src], { type: 'text/javascript' });
  await import(URL.createObjectURL(blob));
  ```

  Typical compression ratio is ~4-5x (a 3 MB raw bundle lands at ~700 KB
  gzipped, ~950 KB base64'd, ~1 MB JSON-encoded -- well under the default
  2 MB PUT cap). Cost is a one-time ~10-20 ms decompression on first use.
  Today only `pdf-extractor.html` (`liteparse`) needs this; the other
  bundle-using tools stay on raw inlining.
- **Pre-deploy syntax check (#1 in "Pre-deploy verification" below) skips
  `type="text/plain"` blocks** the same way it already skips
  `application/ld+json` ones. The bundle is its own build artifact, validated
  by the sub-package's own toolchain (Vite/Rollup error if it doesn't
  compile), not by `node:vm`.

## URL state encoding (`encodeState` / `decodeState`)

Tools that need to remember their state across page reloads or build shareable URLs
(`jwt-debugger.html`, `certificate-decoder.html`, `saml-decoder.html`,
`echo.html`, …) must use the same compression scheme. This keeps URLs short, the calling
convention uniform, and gives any future responder script a known wire format to inflate.

### Where the encoded blob goes

**Default: the URL fragment (`#<encoded>`).**

- The fragment is never sent to the server, so it's safe for sensitive content (tokens,
  PEMs, SAML payloads). It also stays out of server / proxy logs.
- The same encoded value powers both live debounced state and the Share button - there
  is no separate share format.
- Live edits use `history.replaceState(null, '', '#' + encoded)`; the Share button
  copies the page URL with the same fragment.

**Exception: when a responder script must read the state server-side**, the share URL
uses a `?c=<encoded>` query parameter (browsers strip `#` before sending the request).
Today this applies only to `echo.html`, which still uses `#<encoded>` for its in-page
configurator and `?c=<encoded>` for the share button (the responder reads
`context.query.c` and synthesises the configured response).

### Wire format

After URL-safe base64 decoding:

```
| 4 bytes      | N bytes                                |
| ulen (LE u32)| deflate-raw of UTF-8(JSON|raw string)  |
```

The 4-byte uncompressed-length prefix is included even for browser-only tools, so any
future responder script can use a pure-JS inflater (e.g. `tiny-inflate`, like
`echo.html`'s responder) without changing the format. `tiny-inflate` requires a
pre-allocated output buffer, and the prefix lets it size that buffer in one step.

### Calling convention

The helpers are **async**, **string-in / string-out**. Callers `JSON.stringify` /
`JSON.parse` themselves at the call site when state is structured. This keeps the
helpers identical across tools regardless of payload shape.

```js
// Stash an object in the URL fragment:
history.replaceState(null, '', '#' + await encodeState(JSON.stringify(state)));

// Read it back:
const raw = await decodeState(location.hash.slice(1));
const state = raw ? JSON.parse(raw) : null;

// For tools whose state is already a string (PEM, raw SAML, etc.), skip the
// JSON.stringify / JSON.parse - pass the string directly.
```

### Canonical snippet (copy verbatim into each tool)

```js
const utf8Enc = new TextEncoder();
const utf8Dec = new TextDecoder();
const toBase64Url = (bytes) => {
    let s = '';
    for (const b of bytes) s += String.fromCharCode(b);
    return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
};
const fromBase64Url = (str) => {
    const b64 = str.replace(/-/g, '+').replace(/_/g, '/');
    const padded = b64 + '==='.slice(0, (4 - b64.length % 4) % 4);
    const bin = atob(padded);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
};
const encodeState = async (text) => {
    const bytes = utf8Enc.encode(text);
    const stream = new Blob([bytes]).stream().pipeThrough(new CompressionStream('deflate-raw'));
    const deflated = new Uint8Array(await new Response(stream).arrayBuffer());
    const out = new Uint8Array(4 + deflated.length);
    new DataView(out.buffer).setUint32(0, bytes.length, true);
    out.set(deflated, 4);
    return toBase64Url(out);
};
const decodeState = async (str) => {
    try {
        const bytes = fromBase64Url(str);
        if (bytes.length < 4) return null;
        const stream = new Blob([bytes.subarray(4)]).stream()
            .pipeThrough(new DecompressionStream('deflate-raw'));
        const inflated = new Uint8Array(await new Response(stream).arrayBuffer());
        return utf8Dec.decode(inflated);
    } catch { return null; }
};
```

### Caveats

- DEFLATE has ~10 bytes of fixed overhead, so very small payloads (under ~50 bytes) get
  slightly longer. Anything text-heavy compresses 2-10x.
- `CompressionStream` / `DecompressionStream` are baseline since Safari 16.4 - matches
  the "evergreen browsers" target stated above.
- Old uncompressed share URLs (e.g. `?pem=…`, `?jwt=…&secret=…`, `?saml=…`) do **not**
  decode under this format. Migrating a tool to the canonical helpers is a clean break:
  pre-existing share links land users on the empty tool. Note this in the tool's commit
  message and move on.
- When a responder needs to read the state, mirror `echo.html`'s pattern: vendored
  `tiny-inflate` + a `ulen` cap (1 MiB is plenty) + bounds-checked source pointer to
  turn malformed input into a clean error response instead of an infinite loop.

## AI-agent surface (`<slug>.skill.md` and `llms.txt`)

Every tool has a companion **AI-agent skill** at `<su-tool-path>.md`, formatted
as a real [Claude Code / Cursor SKILL.md](https://docs.claude.com/en/docs/agents-and-tools/agent-skills/overview):
terse YAML frontmatter (`name` + `description` only) and a rich Markdown body
that an agent reads top-to-bottom to learn how to drive the tool end-to-end
without scraping the HTML UI. The skill is **the same artefact** whether it's
loaded by Claude Code as an installable skill, fetched ad-hoc by an agent
WebFetch call, indexed by an llmstxt.org crawler, or read by a human in a
browser tab.

The canonical reference for the shape and tone is the original Echo skill:
<https://x.secutils.dev/echo/skill.md>. `dev/tools/echo.skill.md` is kept
byte-aligned with that file (modulo the `tools.secutils.dev` host swap) so
porting between repos stays trivial.

### URL shape

| Surface         | URL                                | Content type    | Source on disk                                                                                               |
|-----------------|------------------------------------|-----------------|--------------------------------------------------------------------------------------------------------------|
| Tool page       | `https://{{TOOLS_HOST}}/<path>`    | `text/html`     | `dev/tools/<name>.html`                                                                                      |
| Per-tool skill  | `https://{{TOOLS_HOST}}/<path>.md` | `text/markdown` | `dev/tools/<name>.skill.md`                                                                                  |
| Aggregate index | `https://{{TOOLS_HOST}}/llms.txt`  | `text/markdown` | generated at deploy time from .html metadata; also the destination of `/`'s `Accept: text/markdown` redirect |

Two **separate responders** per tool: one for the HTML body, one for the markdown.
This avoids fragile content-negotiation, keeps the HTML body under
`html-minifier-terser` while the skill is shipped untouched, and makes the URL
shape obvious to crawlers and to humans (`/jwt` vs `/jwt.md`).

The aggregate `/llms.txt` follows the [llmstxt.org](https://llmstxt.org/) convention -
a short Markdown index keyed by tool name, with promoted tools first. It's
regenerated by `dev/tools/deploy.ts` every time the deploy runs (the resulting
`dev/tools/llms.txt` is git-ignored). See **"How the deploy pipeline handles
skills"** below for the metadata source of truth.

### SKILL.md shape

Each `<name>.skill.md` must be a valid SKILL.md: minimal frontmatter, the
detail lives in the body so a skill loader can install it and an LLM can read
it cold without parsing custom YAML.

```markdown
---
name: jwt-debugger
description: >-
  Decode, verify, and sign HMAC JSON Web Tokens with the Secutils.dev JWT
  Debugger. Build a one-click prefilled URL the user can open by encoding
  `{j: <jwt>, s: <secret>}` into the fragment of
  https://tools.secutils.dev/jwt#{encoded}. Trigger when the user asks to
  "decode this JWT", "verify a JWT signature", inspect a Bearer token, or
  anything that names secutils.dev/jwt.
---

# JWT Debugger (Secutils.dev)

<one paragraph: what the tool does, where state lives, what's out of scope>

## Inputs

| Field | Type | Default | Notes |
| ...   | ...  | ...     | ...   |

## Wire format

<the deflate-raw / ulen / base64url pipeline, copy-paste from echo>

## How to produce the URL

<runnable Node ≥ 18 snippet, no deps, takes argv JSON, prints the full URL>

## After producing

<UX guidance: one sentence summary, fenced block, don't paraphrase>

## Caveats

- <secrets in the fragment, content-type defaults, scope limitations, ...>
```

Frontmatter rules (all that the loader contracts on):

- `name`: a stable kebab-case slug. **Does not have to match the file slug** -
  e.g. `echo.skill.md` declares `name: mock-response` to align with the
  installed Anthropic skill of the same name.
- `description`: a multi-line scalar (use the `>-` folded form). Pack it with
  natural-language trigger phrases - this is what the loader matches against
  user prompts to decide whether to surface the skill. Mention the live URL
  shape inline so an agent that loads only the frontmatter still has enough
  to act.

Body rules (convention, not contract - but every consumer benefits):

- Heading levels are flat (`#` for the title, `##` for sections). No `###`
  unless you're really nesting.
- The "How to produce the URL" snippet must be **runnable as-is** with Node
  ≥ 18 and zero deps. Pass state as `argv[1]` so the shell quoting story
  stays simple. Always `console.log` the **full** URL, never just the
  fragment.
- Tools without URL-state deep-linking (`mock-saml-idp`) skip the wire format
  / encoder sections and use a "How to direct the user" section instead - see
  `mock-saml-idp.skill.md` for the template.

### How the deploy pipeline handles skills

[`dev/tools/deploy.ts`](deploy.ts) iterates `dev/tools/*.skill.md`,
substitutes `{{TOOLS_HOST}}`, and PUTs each file as `text/markdown` to the
corresponding `_MD`-suffixed responder. The skill body is **opaque** to the
deploy script - it doesn't try to parse beyond the host substitution.

The `llms.txt` aggregate is built from a separate metadata source: the
`<meta name="su-tool-name">`, `su-tool-path`, `su-tool-description`, and
`su-tool-promote` tags in the corresponding `<slug>.html`. This keeps the
registry honest:

- The HTML's meta tags are also consumed by `scripts/tools-check.ts`,
  `e2e/tools/registry.ts`, and the marketing site, so there's exactly one
  canonical place to declare a tool's name / path / description / promotion.
- The skill's frontmatter stays minimal (skill-loader-friendly) and doesn't
  drift out of sync with the page.
- A tool only appears in `llms.txt` (and in `sitemap.xml` /
  `agent-skills/index.json`) if **all** of (a) it has
  `<meta name="su-tool-promote" content="true">` (see "Promotion" above),
  (b) it has a sibling `<slug>.skill.md` on disk, and (c) the corresponding
  `_MD` responder ID is configured in `.env`. (a) hides niche tools from
  every aggregate; (b) and (c) together prevent 404 `.md` URLs during
  incremental rollouts.

Per-tool environment variables follow this convention:

```bash
SECUTILS_HTML_APP_RESPONDER_ID_JWT_DEBUGGER=...    # serves /jwt
SECUTILS_HTML_APP_RESPONDER_ID_JWT_DEBUGGER_MD=... # serves /jwt.md
SECUTILS_HTML_APP_RESPONDER_ID_LLMS_TXT=...        # serves /llms.txt
```

A skill source whose `_MD` responder ID is missing is skipped with a yellow
warning (same staged-rollout behaviour as for HTML responders), and is
omitted from `llms.txt`.

### Cross-cutting discovery surfaces (`/robots.txt`, `/sitemap.xml`, `/.well-known/agent-skills/index.json`, Link headers)

Beyond the per-tool `.md` skills and the `llms.txt` aggregate, the deploy
script ships four additional artefacts that the [isitagentready.com](https://isitagentready.com)
checklist asks every agent-friendly site to publish. None of them require any
per-tool authoring; they are derived 1:1 from the same HTML registry +
`*.skill.md` directory listing as `llms.txt`.

| URL                                    | Content type            | Source of truth                              | Responder env var                                   |
|----------------------------------------|-------------------------|----------------------------------------------|-----------------------------------------------------|
| `/robots.txt`                          | `text/plain`            | `buildRobotsTxt()` in `deploy.ts`            | `SECUTILS_HTML_APP_RESPONDER_ID_ROBOTS_TXT`         |
| `/sitemap.xml`                         | `application/xml`       | `buildSitemapXml()` in `deploy.ts`           | `SECUTILS_HTML_APP_RESPONDER_ID_SITEMAP_XML`        |
| `/.well-known/agent-skills/index.json` | `application/json`      | `buildAgentSkillsIndex()` in `deploy.ts`     | `SECUTILS_HTML_APP_RESPONDER_ID_AGENT_SKILLS_INDEX` |
| `Link:` headers on `/`                 | (HTTP response headers) | hard-coded `indexLinkHeaders` in `deploy.ts` | (no extra responder; pinned via index settings)     |

#### `/robots.txt`

A single text file containing:

- A wildcard `User-agent: * / Allow: /` rule (we have nothing private here).
- Explicit `Allow: /` entries for every named AI crawler we know about
  (GPTBot, OAI-SearchBot, ChatGPT-User, ClaudeBot, Claude-Web, anthropic-ai,
  Google-Extended, PerplexityBot, Perplexity-User, Applebot-Extended,
  cohere-ai, CCBot, Bytespider, Diffbot, DuckAssistBot, Meta-ExternalAgent,
  Amazonbot, FacebookBot). The wildcard already covers them, but being
  explicit is a clear "we welcome agent traffic" signal.
- A [Content Signals](https://contentsignals.org/) directive declaring that
  AI training, search indexing, and AI input (RAG / agent retrieval) are all
  welcome: `Content-Signal: ai-train=yes, search=yes, ai-input=yes`.
- A `Sitemap:` reference pointing at `/sitemap.xml`.

To add a new AI crawler, append to the `aiAgents` array in `buildRobotsTxt`.

#### `/sitemap.xml`

Standard sitemaps.org 0.9 XML with one `<url>` per public surface:
the index, every promoted tool's `<path>` and `<path>.md`, every
non-promoted tool's `<path>` and `<path>.md`, plus the aggregate
`/llms.txt` and `/.well-known/agent-skills/index.json`. `<lastmod>` is set
to today's date on every deploy; `<changefreq>` is `weekly` for everything
because the tools really do change at roughly that cadence and search
engines respect it as a hint, not a contract.

#### `/.well-known/agent-skills/index.json`

[Cloudflare's Agent Skills Discovery RFC v0.2.0](https://github.com/cloudflare/agent-skills-discovery-rfc)
shape: `$schema` URI (pinned to the canonical
`https://schemas.agentskills.io/discovery/0.2.0/schema.json` - the spec
requires strict clients to match it exactly) plus a `skills` array where
each entry has:

- `name` - the **frontmatter `name:` value from the SKILL.md**, not the
  file slug. This is the canonical Agent Skills identifier (e.g.
  `pem-certificate-decoder`, `mock-response`); the slug (`pem`, `echo`) is
  a deploy-time path concern and would diverge from the promo site's
  `/.well-known/agent-skills/index.json`, which keys off the same field.
  `deploy.ts` parses the frontmatter at index build time and **fails the
  deploy** if any skill is missing a `name:` or if two skills collide on
  it - agents cache by name, so a collision corrupts that cache.
- `type: "skill-md"` - the v0.2.0 RFC requires `"skill-md"` or
  `"archive"`; strict clients silently skip unrecognized values. Earlier
  deploys used `"skill"`, which would have made every entry invisible to
  a literal RFC implementation.
- `description` - mirrors the HTML's `su-tool-description` `<meta>` so
  marketing/SEO/agent copy stays in sync from one source.
- `url` - the live `<path>.md` URL.
- `digest: "sha256:<hex>"` - per the RFC's "Integrity and Verification"
  section. The hash is computed from the **substituted** Markdown body
  that actually ships, so an agent that's already cached the skill can
  detect updates with a single GET. Earlier deploys emitted a bare
  `sha256: <hex>` field instead, which strict clients would not recognise.

#### `Link:` headers on `/`

The index responder PUTs a single RFC 8288 `Link` response header carrying
three comma-separated link-values, so any agent that fetches just `/` gets
pointers to the discovery surfaces in response headers, no body parsing
required:

```
Link: </llms.txt>; rel="describedby"; type="text/markdown",
      </.well-known/agent-skills/index.json>; rel="describedby"; type="application/json",
      </sitemap.xml>; rel="sitemap"; type="application/xml"
```

We combine into one header rather than sending three because the responder's
HeaderMap-style serializer collapses duplicate `Link:` entries (last write
wins). RFC 8288 §3 explicitly allows this combined form as long as the order
of link-values is preserved.

Per-tool responders deliberately do not carry these headers -- the index is
the single hub agents are expected to land on first.

### Markdown content negotiation (`Accept: text/markdown`)

Every HTML tool responder also honours `Accept: text/markdown` content
negotiation: an agent that sends a request with `Accept: text/markdown` (or
any Accept value that contains `text/markdown` and does not start with
`text/html`) gets a `302` redirect to the corresponding `<slug>.md` (or
`/llms.txt` for the index page). Browsers, `curl --compressed` (which
sends `Accept: */*`), and any standard HTML client see no behaviour change
because their Accept value starts with `text/html` (or is `*/*`).

The redirect is wired up by a tiny prelude (~250 B minified) that
`deploy.ts` injects automatically into every HTML responder's `script`
setting at deploy time. See `wrapWithMdNegotiation()` in
[`dev/tools/deploy.ts`](deploy.ts). The prelude composes with any existing
`@su:responder-script` -- it runs first, may `return` a 302, and otherwise
falls through to the user script's own return value. There is no
per-tool authoring required.

The redirect is pinned to a tool only when its sibling `.md` is actually
deployable (its `_MD` responder ID is configured for tool pages, or
`_LLMS_TXT` for the index). This prevents Accept-negotiated requests from
landing on a 404 during an incremental rollout. The response carries
`Vary: Accept` so any caching proxy keeps the HTML and Markdown variants
distinct.

### Skill link button (header) - per-tool pages only

Each per-tool HTML carries a uniform header button so humans can discover the
skill file (the URL is otherwise invisible to non-AI eyes). The href is set at
runtime from `location.pathname` so the markup is identical across tools:

```js
const path = location.pathname.replace(/\/$/, '') || '/';
if (path !== '/') {
    document.getElementById('skillLink').href = path + '.md';
}
```

The index page (`/`) deliberately omits this chip. Agents have five
overlapping ways to find `/llms.txt` from `/` without parsing the HTML:
`Accept: text/markdown` content negotiation (302 to `/llms.txt`), the `Link:`
response header on `/`, `/robots.txt`'s `Sitemap:` reference, `/sitemap.xml`,
and `/.well-known/agent-skills/index.json` - and `/llms.txt` itself sits at a
[llmstxt.org](https://llmstxt.org/) well-known path that AI crawlers know to
fetch. Humans rarely want a directory-of-all-skills URL, so the chip's primary
human use case (right-click → copy → paste into a chat: "here is the skill
for this tool") doesn't apply at the index. Per-tool pages keep it because
that human use case IS real there.

#### Markup (copy verbatim)

Place inside `.header-right`, before the `<button class="theme-toggle">`:

```html
<a id="skillLink" class="skill-link" href="#" target="_blank" rel="noopener"
   title="View AI agent skill (opens in new tab)"
   aria-label="View AI agent skill (opens in new tab)">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.582a.5.5 0 0 1 0 .962L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z"/>
        <path d="M20 3v4"/><path d="M22 5h-4"/><path d="M4 17v2"/><path d="M5 18H3"/>
    </svg>
    <span>Skill</span>
</a>
```

The icon is the Lucide `sparkles` glyph - the most widely recognised AI
affordance in current UI design (matches Anthropic, OpenAI, Cursor, etc.).

#### CSS (copy verbatim)

Place next to the existing `.theme-toggle` rules:

```css
.skill-link { height: 36px; display: inline-flex; align-items: center; gap: 6px; padding: 0 12px;
              border: 1px solid var(--border); border-radius: 18px; background: var(--surface);
              color: var(--text-muted); font: 12px var(--font); text-decoration: none;
              transition: all .15s; cursor: pointer; }
.skill-link:hover { color: var(--text); border-color: var(--text-muted); background: var(--surface-hover); }
.skill-link svg { width: 14px; height: 14px; fill: none; stroke: currentColor; }
```

The fixed `36px` height matches the round theme toggle, so the two header
chrome controls line up on the same baseline. Inside the
`@media (max-width: 640px)` block, collapse the label so the button stays
compact on mobile:

```css
.skill-link span { display: none; }
.skill-link { padding: 0 10px; }
```

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
<link rel="canonical"        href="https://{{TOOLS_HOST}}{{su-tool-path}}">

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
  "More free tools bottom CTA" below). This is the **only** related-tools surface
  on the page - it carries both the SEO internal-linking value (one link from
  every leaf back to the index) and the human / agent discovery affordance.

  Earlier revisions of these tools shipped an additional `<nav class="su-related">`
  list of every other promoted tool sitting between `</main>` and `<footer>`.
  That block is now obsolete: it duplicated what the index already does (and
  did so with stale, hand-curated copy that drifted out of sync), and visually
  competed with the brighter yellow CTA. When migrating an older tool to the
  banner, **delete** the `<nav class="su-related">` element and the matching
  `.su-related*` CSS rules.

### Per-tool OG image

Every tool ships a 1200x630 OG image at
`https://secutils.dev/docs/img/og/og-<slug>.png` (and a sibling
`og-<slug>-light.png` for light-themed previews). These are auto-generated; do
not paint them by hand. See **OG image generation** below.

## OG image generation

OG images are rendered at deploy time by the existing Playwright stack. Source
template: [`dev/tools/og-template.html`](og-template.html), parameterised via
URL query strings (`name`, `path`, `desc`, `accent`, `icon`, `theme`, `host`).
The driver spec [`e2e/tools/og.spec.ts`](../../e2e/tools/og.spec.ts) iterates
over [`e2e/tools/registry.ts`](../../e2e/tools/registry.ts) and writes both a
dark and a light PNG per tool into
`components/secutils-docs/static/img/og/`. Docusaurus serves `static/*`
verbatim, so the final URLs are stable and unhashed.

```bash
# Regenerate every OG image (14 PNGs: dark + light × 7 tools)
make tools-og

# Verify byte-stability (re-runs N times, checks the files do not change)
make tools-og-loop RUNS=5
```

The same stability guarantees as the docs screenshot suite apply: pre-screenshot
DOM stabilization in `goto()`, sticky-pixel re-encoding to absorb sub-pixel
anti-aliasing jitter, fixed viewport at exactly 1200x630 so no scaling
math is involved. Adding a new tool is a one-row diff in `registry.ts` plus a
`make tools-og` to materialise the PNGs.

## Brand Colors (from Elastic EUI theme-borealis)

### Dark theme (`:root, [data-theme="dark"]`)
| Variable          | Value     | Source                                                                |
|-------------------|-----------|-----------------------------------------------------------------------|
| `--bg`            | `#141519` | EUI dark background                                                   |
| `--surface`       | `#1d1e24` | EUI dark header/card surface                                          |
| `--surface-hover` | `#2c2d33` | EUI dark hover                                                        |
| `--border`        | `#343741` | EUI dark border                                                       |
| `--text`          | `#dfe5ef` | EUI dark text                                                         |
| `--text-muted`    | `#98a2b3` | EUI dark subdued text                                                 |
| `--primary`       | `#fed047` | Secutils yellow                                                       |
| `--primary-hover` | `#fdc615` | Secutils yellow hover                                                 |
| `--primary-text`  | `#642340` | Secutils maroon (text on yellow bg)                                   |
| `--accent`        | `#642340` | Secutils maroon                                                       |
| `--badge-bg`      | `#2B394F` | EUI breadcrumb bg (dark) - `colors.backgroundLightText` = blueGrey120 |
| `--badge-text`    | `#98A8C3` | EUI breadcrumb text (dark) - `colors.textSubdued` = blueGrey55        |

### Light theme (`[data-theme="light"]`)
| Variable          | Value     | Source                                                                |
|-------------------|-----------|-----------------------------------------------------------------------|
| `--bg`            | `#f5f7fa` | EUI light background                                                  |
| `--surface`       | `#ffffff` | White                                                                 |
| `--surface-hover` | `#f1f3f5` | EUI light hover                                                       |
| `--border`        | `#d3dae6` | EUI light border                                                      |
| `--text`          | `#343741` | EUI light text                                                        |
| `--text-muted`    | `#69707d` | EUI light subdued text                                                |
| `--primary`       | `#fed047` | Secutils yellow                                                       |
| `--primary-hover` | `#fdc615` | Secutils yellow hover                                                 |
| `--primary-text`  | `#642340` | Secutils maroon                                                       |
| `--accent`        | `#642340` | Secutils maroon                                                       |
| `--badge-bg`      | `#E3E8F2` | EUI breadcrumb bg (light) - `colors.backgroundLightText` = blueGrey20 |
| `--badge-text`    | `#505F79` | EUI breadcrumb text (light) - `colors.textSubdued`                    |

## Typography

- **Body font**: `'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif`
- **Mono font**: `'Roboto Mono', 'SF Mono', 'Fira Code', Consolas, monospace`
- Load from Google Fonts CDN:
  ```html
  <link rel="preconnect" href="https://fonts.googleapis.com">
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@300..700&family=Roboto+Mono:wght@400..700&display=swap" rel="stylesheet">
  ```

### Typography polish (`text-wrap: balance` / `pretty`)

Every tool ships the following single CSS block adjacent to the
`.su-dialog-fineprint` rule (search anchor, present in every file):

```css
/* Typography polish (Baseline; selectors that don't exist in this tool are no-ops). */
.page-title, .panel-label, .su-dialog-header h2, .card-header { text-wrap: balance; }
.su-dialog-body p, .su-more-tools p, .tool-desc, .empty-state-sub, .progress-sub, .error-sub { text-wrap: pretty; }
```

Rules:

- **`text-wrap: balance`** is reserved for short, deliberately-set text:
  page titles, panel labels, dialog headings, card titles. The browser
  balances line lengths so headings never leave a single word on the last
  line. Baseline since 2024-05-13.
- **`text-wrap: pretty`** goes on multi-line body copy where orphans look
  worst: Privacy / Credits dialog paragraphs, the bottom "more tools" promo
  copy, empty-state / progress / error sub-headings. Baseline Newly
  Available since 2025-04 across all three engines.
- **Never** use the global `* { text-wrap: balance; }` shortcut — the
  balancing algorithm runs a binary search on line widths and is expensive
  if applied to every node in the document. The selector list above
  intentionally targets a few dozen elements at most.
- Selectors that don't exist in a given tool (e.g. `.empty-state-sub` is
  absent in `index.html`) are harmless no-ops. Keep the full list verbatim
  so adding a new shared class later doesn't need a sweep across every
  tool.
- No fallback is needed; browsers without support fall back to default
  `wrap` (the current behaviour) automatically. The block is a pure
  progressive enhancement.

## Header

- **Height**: `48px` (matches EUI `EuiHeader`)
- **Padding**: `0 16px`
- **Background**: `var(--surface)` with `border-bottom: 1px solid var(--border)`
- **Position**: `sticky; top: 0; z-index: 100`
- **Layout**: `display: flex; align-items: center; justify-content: space-between`

### Logo (left side)

Use the full Secutils.dev logo SVG (SU icon + "SECUTILS.DEV" text as one SVG). The SVG is from `components/secutils-webui/src/components/logo_with_name.tsx` - a cleaned-up version without Inkscape metadata.

- Wrap in `<a class="logo" href="https://secutils.dev" target="_blank" rel="noopener">`
- SVG height: `24` (viewBox `0 0 98 16`)
- The "SECUTILS.DEV" text path must have `class="logo-text-fill"` so its fill adapts to the theme via CSS: `.logo-svg .logo-text-fill { fill: var(--text); }`
- The SU icon rect is always `fill="#fed047"` and the SU letters are always `fill="#642340"`

### Tool Name Badge (next to logo)

Styled as an EUI application breadcrumb:

```css
.logo-badge {
    display: inline-flex;
    align-items: center;
    padding: 4px 16px;
    border-radius: 4px;
    border: none;
    background: var(--badge-bg);
    color: var(--badge-text);
    font-size: 12px;
    font-weight: 450;
    line-height: 16px;
    white-space: nowrap;
}
```

### Logo SVG

The full Secutils.dev SVG (~5 KB of inline path data) is identical across every
tool. Don't paste it into AGENTS.md - copy it verbatim from
[`dev/tools/index.html`](index.html)'s `<a class="logo">` block. Only the wrapping
`<a>` and the height attribute matter for new tools (height `24` everywhere except
`20` on mobile, controlled by the `.logo-svg` rule in the responsive section).

## Dark/Light Theme Toggle

- Use EUI SVG icons (from `node_modules/@elastic/eui/lib/components/icon/svgs/`), **not** emoji
- CSS controls visibility based on `data-theme` attribute - no JS needed to swap icons
- Button style: `36px` circle, `border: 1px solid var(--border)`, `background: var(--surface)`

### Sun icon SVG (shown in dark mode - "switch to light")
```html
<svg class="icon-sun" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M8.5 15h-1v-2h1v2Zm-3.674-3.107-1.414 1.414-.707-.707 1.414-1.415.707.708Zm8.479.707-.707.707-1.414-1.414.707-.708 1.414 1.415Z"/><path fill-rule="evenodd" d="M8 4a4 4 0 1 1 0 8 4 4 0 0 1 0-8Zm0 1a3 3 0 1 0 0 6 3 3 0 0 0 0-6Z" clip-rule="evenodd"/><path d="M3.005 8.505h-2v-1h2v1Zm12 0h-2v-1h2v1ZM4.82 4.114l-.708.707-1.414-1.414.707-.707L4.82 4.114Zm8.492-.707-1.414 1.414-.708-.707L12.605 2.7l.707.707ZM8.5 3h-1V1h1v2Z"/></svg>
```

### Moon icon SVG (shown in light mode - "switch to dark")
```html
<svg class="icon-moon" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M4.05 12.95A6.982 6.982 0 0 1 2 8c0-1.79.684-3.583 2.05-4.95A6.982 6.982 0 0 1 9 1a1 1 0 0 1 .708 1.707 4.982 4.982 0 0 0-1.465 3.536 4.98 4.98 0 0 0 1.465 3.535 4.98 4.98 0 0 0 3.535 1.465 1 1 0 0 1 .707 1.707A6.981 6.981 0 0 1 9 15a6.983 6.983 0 0 1-4.95-2.05Zm.708-.707A5.983 5.983 0 0 0 9 14c1.535 0 3.07-.586 4.242-1.757a5.98 5.98 0 0 1-4.018-1.545L9 10.485a5.982 5.982 0 0 1-1.758-4.242A5.986 5.986 0 0 1 9 2a5.983 5.983 0 0 0-4.243 1.757A5.98 5.98 0 0 0 3 8l.006.288a5.978 5.978 0 0 0 1.75 3.955Z"/></svg>
```

### CSS for toggle
```css
.theme-toggle { width: 36px; height: 36px; padding: 0; display: flex; align-items: center; justify-content: center; border-radius: 50%; border: 1px solid var(--border); background: var(--surface); color: var(--text-muted); cursor: pointer; transition: all .2s; }
.theme-toggle:hover { background: var(--surface-hover); color: var(--text); }
.theme-toggle svg { width: 16px; height: 16px; fill: currentColor; }
.theme-toggle .icon-sun { display: none; }
.theme-toggle .icon-moon { display: block; }
[data-theme="dark"] .theme-toggle .icon-sun { display: block; }
[data-theme="dark"] .theme-toggle .icon-moon { display: none; }
```

### JS for toggle
```js
(() => {
    const root = document.documentElement;
    const toggle = document.getElementById('themeToggle');
    const setTheme = (t) => {
        root.setAttribute('data-theme', t);
        try { localStorage.setItem('su-tool-theme', t); } catch {}
    };
    toggle.addEventListener('click', () => {
        setTheme(root.getAttribute('data-theme') === 'dark' ? 'light' : 'dark');
    });
    try {
        const saved = localStorage.getItem('su-tool-theme');
        if (saved) setTheme(saved);
        else if (window.matchMedia('(prefers-color-scheme: light)').matches) setTheme('light');
    } catch {}
})();
```

## Buttons

```css
.btn { padding: 7px 14px; height: 29px; border-radius: 8px; border: 1px solid var(--border); background: var(--surface); color: var(--text); font: 13px/1 var(--font); cursor: pointer; transition: all .15s; display: inline-flex; align-items: center; gap: 5px; }
.btn:hover:not(:disabled) { background: var(--surface-hover); border-color: var(--text-muted); }
.btn-primary { background: var(--primary); border-color: var(--primary-text); color: var(--primary-text); font-weight: 500; }
.btn-primary:hover:not(:disabled) { background: var(--primary-hover); border-color: var(--primary-hover); }
.btn-sm { padding: 5px 10px; height: 24px; font-size: 12px; }
.icon-btn { padding: 4px; border: none; background: none; color: var(--text-muted); cursor: pointer; border-radius: 4px; transition: all .15s; display: inline-flex; align-items: center; justify-content: center; }
.icon-btn:hover { color: var(--text); background: var(--surface-hover); }
.icon-btn svg { width: 16px; height: 16px; }
```

### Canonical control height: 24 px

The three small controls that share the `.panel-bar` (`.btn-sm`, `.view-tabs`,
`.icon-btn`) **must compute to the same outer height of 24 px** so they line
up across the splitter when one pane has `Share` (a `.btn-sm`) and the other
has `XML | Attributes` (a `.view-tabs` pill). The default `.btn-sm` came in
at 22 px (4 px vertical padding + 1 px border + 12 px line + 1 px border + 4 px),
the `.view-tabs` pill at 24 px (1 px border + 2 px padding + 18 px content + 2 px
padding + 1 px border) and `.icon-btn` at 28 px (6 px padding + 16 px svg + 6 px),
so all three were "centered in a 38 px bar" but their tops/bottoms drifted by
2-6 px, which is visible across the splitter even though the bar height
itself is invariant.

The current values reconcile to 24 px:

- `.btn-sm`: `padding: 5px 10px; height: 24px; font-size: 12px;` (inherits `line-height: 1` from `.btn { font: 13px/1 ... }`) → 5+1+12+1+5 = 24, pinned by `height: 24px`
- `.view-tabs`: `padding: 2px;` + `border: 1px;` + `.view-tab { padding: 3px 10px; font: 12px/1; }` → 1+2+(3+12+3)+2+1 = 24
- `.icon-btn`: `padding: 4px;` + `svg 16x16` → 4+16+4 = 24

**The `height: 24px` on `.btn-sm` (and `height: 29px` on `.btn`) is
load-bearing**, even though the math from padding + border + line-height
already adds up to 24 / 29. Without an explicit `height`, the **`.btn-primary`
variant (which carries a heavier `font-weight` than the regular variant)
renders 1-2 px taller than the outlined regular variant**. The browser
sizes the line box from the largest font metric on the line, and Inter's
heavier cuts have slightly bigger ascender + descender than its regular
(400) cut -- the unitless `line-height: 1` does not clamp the strut, only
the leading. The mismatch is invisible inside a single pane (all primary
buttons look fine next to each other) but jumps out the moment a primary
and a regular button sit side by side in the same row (e.g. `Options` next
to `Parse` in pdf-extractor's PDF panel-bar). Pinning `height` makes both
weights resolve to the same outer size. `box-sizing: border-box` is global
so padding + border sit inside the pinned height; `align-items: center`
keeps the label visually centred regardless of the strut diff.

**`.btn-primary` is pinned at `font-weight: 500` AND `border-color:
var(--primary-hover)`.** Even with the outer `height` pinned so the boxes
are byte-identical in size, the older `font-weight: 600` + same-as-fill
border combination produced a *perceptual* size bump: Helmholtz's
filled-vs-outlined illusion plus the extra ink stroked by a bold label
made the eye read primary buttons as "taller / heavier" than the outlined
regulars sitting in the same row, even though `getBoundingClientRect()`
reported identical pixel heights across Chromium and Firefox. Two
mitigations stack:

1. **`font-weight: 500`** (down from 600) preserves the yellow +
   `--primary-text` colour as the hierarchy signal while collapsing the
   perceived label weight to roughly match the regular variant's 400.
2. **`border-color: var(--primary-text)`** (instead of `var(--primary)`,
   which is the same as the fill) gives the primary button a 1 px dark
   plum edge that matches its label colour -- the same "stamped /
   self-framed" silhouette that outlined `.btn` gets from `var(--border)`
   on `var(--surface)`. The dark plum was chosen after rejecting two
   alternatives: `var(--primary-hover)` (a barely-darker yellow) was too
   low-contrast against the fill to be perceptible at small sizes, and
   the neutral `var(--border)` grey looked invisible in light mode
   (`#d3dae6` on `#fed047`) and like a rendering bug in dark mode.
   `--primary-text` works in both themes because the token resolves to
   the same `#642340` regardless of mode -- the only token in the
   primary palette guaranteed to have enough luminance contrast against
   the yellow fill.

Do **not** revert either change in isolation. The visual regression is
subtle on a single button but obvious in any `.panel-bar` row that mixes
both variants (e.g. pdf-extractor's `Options` next to `Parse`). The
outlier is `mock-saml-idp.html`, whose regular `.btn` is already
`font-weight: 500`; its `.btn-primary` is left at 600 to keep the same
+100 weight delta as the other tools, but it still uses the
`--primary-hover` border for the framing.

The mobile breakpoint that shrinks `.btn-sm` (e.g. to
`padding: 5px 9px; font-size: 11px;` in `md` and
`pdf-extractor`) **must also pin `height: 23px`** to match the
`.view-tab` mobile collapse to 22-23 px. Forgetting the mobile pin leaves
the same primary-vs-regular drift visible at phone widths.

**The `/1` in `font: 12px/1 var(--font)` is load-bearing.** Without it the
`font` shorthand resets `line-height` to `normal` (~1.2-1.4 for Inter), so the
inner pill renders at ~21 px instead of 18 px and the outer view-tabs swells
to 27 px. The misalignment looks like the bar got taller, but the bar is
still 38 px - the pill's content box just outgrew the `.btn-sm` it's sitting
across the splitter from. Always pin `line-height` explicitly on any control
that lives in `.panel-bar`.

Do **not** change one in isolation - touching any of the three requires
checking the other two and the per-tool mobile override (`.btn-sm` shrinks
to 23 px on mobile in `md` via `padding: 5px 9px; font-size:
11px;` - keep the vertical padding at 5 px so it still matches the
`.view-tab` mobile override which collapses font to 11 px → 22 px outer).

**`.panel-bar` must always pin `flex-shrink: 0`.** This is load-bearing
and easy to miss. The bar is a flex item inside the column wrapper
(`.panel`), the column has another sibling (the editor or the decoded
container), and any sibling that contributes a non-trivial flex basis
will *steal vertical space from the bar*. The most common offender is a
`<textarea class="editor-area">` that has `height: 100%; min-height:
400px;` - the percentage resolves against the column height (typically
~700 px), giving the textarea a flex basis of ~700 px. The bar's basis
is only 46 px, so when the column has to allocate space, both items
compete with `flex-shrink: 1` (default) and the bar gets squeezed by
~2-3 px while the textarea takes nearly the whole column. Meanwhile
the *other* pane has its content sized as `flex: 1; min-height: 400px;`
(basis = 0), so its bar is never squeezed and stays at the declared
46 px. Result: the two panel-bars end up with different heights even
though both declare `height: 38px`, and the panes' bodies fall out of
alignment by exactly the squeeze amount.

`flex-shrink: 0` on the bar fixes it permanently regardless of what
sibling sizing convention the editor uses (`flex: 1` vs `height: 100%`).
The wider rule: **any flex item with a fixed `height` whose presence
matters for cross-pane alignment must also pin `flex-shrink: 0`** -
otherwise its declared height is just the basis, not a hard floor.

The mobile media query (where `.grid` collapses to a single column and
panes stack vertically, so cross-pane alignment no longer matters) flips
the bar to `flex-wrap: wrap`. There it must also override the desktop
`height: 38px` to `height: auto; min-height: 38px;` - otherwise the
wrapped second row of controls renders **outside** the bar's 46 px box
(because the desktop `height: 38px` is a hard cap once we've pinned
`flex-shrink: 0` on the bar) and overlaps the editor / preview below.

The **column-stacking breakpoint must be 900 px** for every two-pane tool
(matching JWT). Splitting the viewport in half below ~900 px leaves each
column at ≤440 px of horizontal space, which is too narrow for the
label-plus-actions bar contents - every link button wraps to two lines
and the bar takes up 30-40 % of the visible vertical space. Keep the
header / button-padding tweaks (`.btn { font-size: 12px; }` etc.) on a
separate, smaller breakpoint (typically 640 px) so they only fire at
real phone widths. Two queries, two responsibilities:

```css
/* Stack columns + wrap bar - fires when 2 columns would be too narrow. */
@media (max-width: 900px) {
  .grid { grid-template-columns: 1fr; row-gap: 24px; }
  .splitter { display: none; }
  .panel-bar { flex-wrap: wrap; row-gap: 8px; height: auto; min-height: 38px; }
  .panel-actions { flex-wrap: wrap; justify-content: flex-end; }
}
/* Phone-sized chrome tweaks - fires only at real phone widths. */
@media (max-width: 640px) {
  header { padding: 0 12px; }
  .logo-svg { height: 20px; }
  .logo-badge { font-size: 11px; padding: 2px 7px; }
  .btn { padding: 6px 10px; font-size: 12px; }
  main { padding: 16px 12px; }
  .editor-area { min-height: 200px; }
}
```

### Copy buttons (icon convention)

Every "copy to clipboard" button in every tool uses the same 16-px clipboard
SVG followed by a labelled span. The icon makes the affordance recognisable
even when the label collapses on mobile, and it matches the visual weight of
the `Export` icon in `md`. Either every Copy button has the
icon or none does - picking and choosing per tool produces the inconsistency
that previously made `saml-decoder` / `jwt-debugger` / `echo` look unrelated
to `md`.

```html
<button id="copy-button" class="btn btn-sm" title="Copy ... to your clipboard">
    <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="4" y="4" width="9" height="10" rx="1.5"/><path d="M3 11V3.5A1.5 1.5 0 0 1 4.5 2H10"/></svg>
    <span class="btn-label">Copy</span>
</button>
```

### Action feedback (toast, not button text swap)

Every "thing happened" feedback - Copy, Share, Export errors - uses a
**bottom-right toast**, not a button-label swap. The button keeps its label
and icon stable; the toast carries the success / failure message. Three
reasons:

1. The icon + label combination has a fixed width, so swapping the label
   (`Copy` → `Copied!`) jitters the surrounding action row.
2. A single feedback channel covers success **and** failure (`Failed to copy`
   has no good inline equivalent for an icon-only Export button).
3. Screen readers announce the toast via `role="status" aria-live="polite"`
   without the focus moving.

Standard microcopy (use these literal strings - do not invent variants):

| Action | Success | Failure |
|---|---|---|
| Copy any payload | `Copied to clipboard` | `Failed to copy` |
| Share (URL-state link) | `Share link copied` | `Failed to copy share link` |

Markup, CSS, and helper (copy verbatim into a tool that doesn't have a toast
yet - `echo`, `jwt-debugger`):

```html
<div id="toast" class="toast" role="status" aria-live="polite" style="display:none">
    <span id="toastMsg"></span>
</div>
```

```css
.toast { position: fixed; bottom: 20px; right: 20px; background: var(--surface); color: var(--text); padding: 10px 18px; border-radius: 8px; border: 1px solid var(--border); font-size: 13px; z-index: 200; box-shadow: 0 4px 12px rgba(0,0,0,0.3); display: flex; align-items: center; gap: 8px; animation: toastIn .2s ease; }
@keyframes toastIn { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
```

```js
let toastTimer;
function toast(msg) {
    document.getElementById('toastMsg').textContent = msg;
    const el = document.getElementById('toast');
    el.style.display = 'flex';
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => { el.style.display = 'none'; }, 2000);
}
```

```js
copyBtn.addEventListener('click', async () => {
    try {
        await navigator.clipboard.writeText(value);
        toast('Copied to clipboard');
    } catch {
        toast('Failed to copy');
    }
});
```

## Two-pane layouts (`.panel-bar`)

Tools that show a two-pane editor / output split (`certificate-decoder`,
`saml-decoder`) align the tops of both panel bodies by
giving each header bar a **fixed** height, not `min-height` and not
content-driven padding:

```css
.panel-bar { display: flex; align-items: center; justify-content: space-between; padding: 0 0 8px; gap: 8px; height: 38px; box-sizing: content-box; }
```

The `height: 38px; box-sizing: content-box` is load-bearing. With `min-height`
or no height at all, a bar containing buttons (`Example`, `Clear`, tabs) grows
to ~38 px while a bar containing only an `<h2>` stays at the text's intrinsic
height (~24 px). The two panes' bodies then start at different `Y` coordinates
and the misalignment is visible at every viewport width. A fixed height forces
both bars to the same box regardless of contents.

Keep the `padding: 0 0 8px` (8 px below the bar, none above) so the bar sits
flush against the section heading; the gap above comes from the section's own
padding.

### Action layout inside `.panel-bar`

The bar's two halves are conventional:

- **Left half** - pane label (`<span class="panel-label">`) **or** tab pill
  (`.view-tabs`) when the pane has multiple views. Never both - if there are
  tabs, the label is implicit in the tab name. Keep labels to a single short
  word (`Encoded`, `Markdown`, `Decoded`, `PEM Input`).
- **Right half** - `.panel-actions` containing, in order:
  1. stats text (`.stats-text`, hidden on mobile),
  2. link-style helpers (`.link-btn` for `Example` / `Clear` / `Upload`),
  3. primary buttons (`.btn .btn-sm`, including `Share` on the input pane and
     `Copy` / `Export` on the output pane),
  4. `.icon-btn` toggles **last**, so Fullscreen always sits at the far edge
     of the bar - that position survives mobile wrap, mirrors the convention
     across all tools, and keeps the popover-anchored buttons (`Options`,
     `Export`) flush with the labelled buttons they belong to.

### Tab pill (`.view-tabs`)

Output panes that need to switch between rendered views (e.g. XML vs.
Attributes in `saml-decoder`) use a segmented pill, not a border-bottom tab
bar. The pill
lives **inside** the panel-bar (left half), keeping the bar height invariant
and matching the `.btn-sm` visual weight on the right.

```css
.view-tabs { display: inline-flex; gap: 2px; padding: 2px; background: var(--surface); border: 1px solid var(--border); border-radius: 6px; }
.view-tab  { padding: 3px 10px; border: none; background: transparent; color: var(--text-muted); font: 12px var(--font); font-weight: 500; cursor: pointer; border-radius: 4px; transition: all .15s; }
.view-tab:hover  { color: var(--text); }
.view-tab.active { background: var(--surface-hover); color: var(--text); font-weight: 600; }
```

```html
<div class="view-tabs" role="tablist" aria-label="Decoded view">
    <button class="view-tab active" data-tab="xml" role="tab" aria-selected="true">XML</button>
    <button class="view-tab"        data-tab="attributes" role="tab" aria-selected="false">Attributes</button>
</div>
```

The matching JS toggles `.active` on the buttons and `.tab-content.active` on
the panels, and mirrors `aria-selected` so screen readers track the state.

### Fullscreen toggle (`.icon-btn` + `:fullscreen`)

The output pane's `.panel-actions` carries a 16-px icon button that requests
fullscreen on the **wrapper** that holds both the panel-bar and the pane
content (not the inner content alone - fullscreening only the content hides
the tabs and actions). Give that wrapper an id (`#previewPane`,
`#decoded-pane`, …) and target it from CSS:

```css
.icon-btn { padding: 6px; border: none; background: none; color: var(--text-muted); cursor: pointer; border-radius: 4px; transition: all .15s; display: inline-flex; align-items: center; justify-content: center; }
.icon-btn:hover { color: var(--text); background: var(--surface-hover); }
.icon-btn svg  { width: 16px; height: 16px; }

#previewPane:fullscreen { background: var(--bg); padding: 16px; display: flex; flex-direction: column; overflow: hidden; }
#previewPane:fullscreen .output-panel { flex: 1; min-height: 0; }
```

The button has two SVGs (enter / exit), one of which is `display:none` at any
time. A single `fullscreenchange` listener on `document` flips them - handle
the toggle here, not in the click handler, so the icon stays correct when the
user exits via Esc.

```js
fullscreenBtn.addEventListener('click', async () => {
    if (!document.fullscreenElement) await previewPane.requestFullscreen();
    else await document.exitFullscreen();
});
document.addEventListener('fullscreenchange', () => {
    const entering = !!document.fullscreenElement;
    enterIcon.style.display = entering ? 'none' : 'block';
    exitIcon.style.display  = entering ? 'block' : 'none';
});
```

## "More free tools" bottom CTA

Every per-tool page (not the index) carries a small yellow-accent banner as the
**last child of `<main>`** that points back to the tools index. It serves two
audiences at once: a human visitor who came from a search result discovers
sibling tools without leaving the page, and the marketing site / SEO graph
gets a hub-and-spoke of internal links between every tool and the index.

### Markup (copy verbatim)

Place as the last element inside `<main>`, after the tool's primary content
section. Do **not** put it between `</main>` and `<footer>` - it must be inside
`<main>` so it inherits the same content padding and doesn't double-pad against
the footer.

```html
<aside class="su-more-tools" aria-label="More free tools">
    <p>Other free, no-signup Secutils.dev tools for {{categories}} and more - <a href="https://{{TOOLS_HOST}}/">Browse all tools &rarr;</a></p>
</aside>
```

`{{categories}}` is a hand-curated short list excluding the current tool
(e.g. `JWT, SAML, certificates, Markdown` on echo). Keep it plain text, no
em-dashes, single hyphen with spaces between the description and the call-to-
action link.

### CSS (copy verbatim)

Place next to the existing `.su-footer` rules:

```css
.su-more-tools { margin: 8px 0 0; padding: 12px 18px; text-align: center; border: 1px solid rgba(254, 208, 71, 0.35); border-radius: 12px; background: rgba(254, 208, 71, 0.06); font: 13px/1.55 var(--font); color: var(--text); transition: border-color .25s, background-color .25s, color .25s; }
.su-more-tools p { margin: 0; }
.su-more-tools a { color: var(--primary); font-weight: 700; text-decoration: none; white-space: nowrap; }
.su-more-tools a:hover { color: var(--primary-hover); text-decoration: underline; }
@media (max-width: 600px) { .su-more-tools { padding: 12px 14px; } .su-more-tools a { white-space: normal; } }
```

The accent yellow at low opacity (border 35 %, fill 6 %) is intentional: it
matches the brand colour without competing with the tool's own primary action
buttons. The `8px 0 0` margin keeps the banner tight to the section above
(main's padding handles the gap to the footer).

### Why this lives only on tool pages, not the index

The index already IS the "more free tools" page - adding the same banner there
would link `/` -> `/`. The same logic applies to the marketing site's home: it
has its own `#free-tools` card section, so no banner is needed there either.

## Footer

There are two different footer patterns depending on whether the page has the Secutils header or not:

### Tool app pages (have the Secutils logo header)

Since branding is already in the header, the footer should contain a **short description of the tool** - not a "Powered by" watermark. Use `<p>` text, no logo repetition. Every footer also carries a **Privacy** link and (when the tool ships any third-party browser-runtime JS) a sibling **Credits** link, both `<button>` elements that open the canonical dialogs - see "Privacy dialog" and "Credits dialog" below. The links are `<button>` rather than `<a href="#privacy">` so they don't pollute history or the URL fragment (the fragment is reserved for tool state, see "URL state encoding" above).

Two-line layout: the tool description on the first line, the Privacy / Credits links demoted to a smaller, dimmer second line so they read as "fine print" rather than competing with the description. When both links are present, separate them with a middle dot (`&middot;`) wrapped in `<span aria-hidden="true">` so screen readers skip the visual ornament.

```html
<footer class="su-footer">
    <p>A single-file tool description goes here.</p>
    <p class="su-footer-fineprint">
        <button type="button" class="su-footer-link" id="privacyOpen">Privacy</button>
        <span aria-hidden="true"> &middot; </span>
        <button type="button" class="su-footer-link" id="creditsOpen">Credits</button>
    </p>
</footer>
```

Tools whose browser side ships **zero** third-party JS (today: `index.html`, `echo.html` - tiny-inflate is server-side only) omit the Credits link and the middle-dot separator, leaving the fineprint as a single Privacy button.

```css
.su-footer {
    text-align: center;
    padding: 16px;
    border-top: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 0.8rem;
}
.su-footer p { margin: 0; }
.su-footer-fineprint { margin-top: 6px !important; font-size: 0.7rem; opacity: 0.75; }
.su-footer-link { background: none; border: none; padding: 0; color: inherit; font: inherit; cursor: pointer; text-decoration: underline; text-underline-offset: 2px; }
.su-footer-link:hover { color: var(--text); }
```

`opacity: 0.75` (not a darker `color`) is intentional: it dims **both** the muted text and the inherited link colour in one go, and stays correct across the light/dark theme swap without needing per-theme overrides. The `!important` on `.su-footer-fineprint`'s `margin-top` only exists to override the universal `* { margin: 0; }` reset declared at the top of every tool's stylesheet.

### Generated/exported output files (no Secutils header - e.g. downloaded HTML from Markdown → HTML tool)

Since there is no header with branding, include a **"Powered by Secutils.dev" watermark footer** - subtle, non-distracting, links to `https://secutils.dev`:

```html
<footer class="su-watermark">
  <a href="https://secutils.dev" target="_blank" rel="noopener">
    <svg width="16" height="16" viewBox="0 0 16 16" xmlns="http://www.w3.org/2000/svg">
      <!-- SU initials logo (small) -->
    </svg>
    <span>Powered by <strong>Secutils.dev</strong></span>
  </a>
</footer>
```
Watermark CSS: `text-align: center; padding: 32px 24px; opacity: 0.6; font-size: 12px; border-top: 1px solid var(--border);`

### Common rules for generated output files

1. **Include dark/light mode toggle** with the same EUI SVG icons (sun/moon)

2. **Use Inter + Roboto Mono fonts** (loaded from Google Fonts CDN)

3. **Use Secutils brand accent colors** (`#fed047` yellow, `#642340` maroon) for links, progress bar, blockquote borders, etc.

## Analytics (Plausible)

Every tool HTML file (including `index.html`) carries the same privacy-friendly
[Plausible](https://plausible.io/) snippet. The snippet lives in `<head>`, placed
**immediately after the Google Fonts `<link>`** and before the inline `<style>`
block (per the [Plausible integration guides](https://plausible.io/docs/integration-guides)).
The matching `<script type="application/ld+json">` SEO block stays where it is - it
goes earlier in `<head>`, between the meta tags.

### Markup (copy verbatim)

```html
<!-- Privacy-friendly analytics by Plausible -->
<script defer src="https://tools.secutils.dev/js/script.js" fetchpriority="low"></script>
<script>
    window.plausible = window.plausible || function () { (plausible.q = plausible.q || []).push(arguments) };
    plausible.init = plausible.init || function (i) { plausible.o = i || {} };
    plausible.init();
</script>
```

Three load-bearing details:

- **Script URL is first-party** (`https://tools.secutils.dev/js/script.js`, same
  host as the page). Bypasses third-party adblockers that filter `plausible.io`
  or generic analytics domains, and piggybacks on the existing connection pool.
  The host is reverse-proxied to Plausible upstream by the same infra that
  serves the tools.
- **`defer`, not `async`.** `defer` keeps the script's execution ordered
  relative to the inline init shim (which runs in document order after parsing
  finishes) and avoids the tiny race where the queue stub might run before the
  loader is ready. Both work in practice; `defer` is the conservative choice
  for an in-`<head>` placement.
- **`fetchpriority="low"`.** Analytics is never LCP-critical; the hint tells
  the browser to keep fonts and the first meaningful render ahead of the
  Plausible loader in the network queue. Pairs with `defer` (the priority
  hint applies to the fetch, the timing hint applies to execution). Same
  attribute is applied to every heavy third-party library (see "Resource
  priority" below).
- **`init()` form without `data-domain`.** Plausible auto-derives the domain
  from `location.hostname`, so a single snippet works on every page and the
  dashboard automatically attributes events to `tools.secutils.dev`. The
  inline shim queues any `plausible(...)` calls made before the loader
  arrives, so future custom events (e.g. `plausible('Copy share link')`)
  buffer cleanly.

### How the deploy pipeline treats the snippet

`html-minifier-terser` strips the `<!-- Privacy-friendly analytics by Plausible -->`
comment via `removeComments: true` and re-emits both `<script>` tags as-is (the
`src="..."` script tag is preserved, the inline init shim goes through
`minifyJS`). No special handling in [`deploy.ts`](deploy.ts) is required.

### Why inline-HTML rather than the dynamic-injection pattern used on `secutils.dev`

The main `secutils.dev` marketing site injects the same Plausible script
dynamically from a TypeScript entry. That works because its Parcel build
bundles the entry into a single JS file. The tools are single static HTML
files with no per-page build step beyond `html-minifier-terser`, so inlining
the snippet is simpler, deterministic, survives minification, and gives
every page the analytics loader before the inline init shim runs (rather
than after a paint).

## Privacy dialog (footer)

Every tool footer carries a **Privacy** button that opens a native `<dialog>`
explaining (1) tool state stays in the browser and (2) what Plausible
collects. The dialog is the user-facing complement of the analytics snippet
above: every tool that runs Plausible discloses Plausible.

### Why a native `<dialog>`

`<dialog>.showModal()` gives Escape-to-close, focus trapping, `role="dialog"`,
a `::backdrop` pseudo-element, and document inertness for free - no library,
no manual ARIA, no keyboard-trap helper. Supported in every evergreen
browser. The dialog does not close on backdrop click by default; that
matches the "short, action-required modal" convention and avoids accidental
dismissals on touch devices. If a tool ever needs backdrop-click dismissal,
wire it up locally; do not add it to the canonical snippet.

**Centering is `inset: 0; margin: auto;` plus a `max-height` cap.** The native
user-agent stylesheet only resolves `margin: auto` horizontally because no
`top`/`bottom` are set on the modal-positioned dialog; adding `inset: 0` gives
both axes an anchor so `margin: auto` distributes the remaining space evenly,
vertically and horizontally. `max-height: calc(100% - 32px)` keeps a 16 px
breathing room at top/bottom on short viewports (e.g. landscape phone) and
lets the dialog body scroll instead of overflowing the viewport. Without the
`max-height`, the dialog could exceed the viewport and the bottom margin
would collapse, breaking the vertical centering.

### Markup (copy verbatim)

Place as the **last child of `<body>`**, after `</footer>` and before the
final `<script>` block. The copy is intentionally generic so the same block
ships unchanged across every tool (the dialog enumerates payload types from
every tool, not just the current page's):

```html
<dialog id="privacyDialog" class="su-dialog" closedby="any" aria-labelledby="privacyDialogTitle">
    <header class="su-dialog-header">
        <h2 id="privacyDialogTitle">Privacy</h2>
        <button type="button" class="su-dialog-close" id="privacyClose" aria-label="Close">
            <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 3l10 10M13 3L3 13"/></svg>
        </button>
    </header>
    <div class="su-dialog-body">
        <p><strong>Your data stays in your browser.</strong> These tools run entirely client-side. Tokens, PEMs, SAML payloads, Markdown source, and mock-response bodies are never sent to the Secutils.dev server. State that needs to survive a reload (or be shared) lives in the URL fragment (<code>#&hellip;</code>), which browsers never transmit to the server.</p>
        <p><strong>Anonymous usage analytics.</strong> We use <a href="https://plausible.io/" target="_blank" rel="noopener noreferrer">Plausible Analytics</a>, a privacy-first, GDPR-compliant tool, to collect aggregate usage data. No cookies, no personal data, no individual tracking. The data is limited to top pages, referral sources, visit duration, and device-class metadata (device type, OS, country, browser). Full details in the <a href="https://plausible.io/data-policy" target="_blank" rel="noopener noreferrer">Plausible Data Policy</a>.</p>
        <p class="su-dialog-fineprint">See the full <a href="https://secutils.dev/privacy" target="_blank" rel="noopener noreferrer">Secutils.dev privacy policy</a> for details on the wider service.</p>
    </div>
</dialog>
```

The matching footer button is documented in the "Footer" section above
(every footer carries `<button class="su-footer-link" id="privacyOpen">Privacy</button>`).

### CSS (copy verbatim)

Place next to the existing `.su-footer` rule:

```css
.su-dialog { max-width: 520px; width: calc(100% - 32px); max-height: calc(100% - 32px); inset: 0; margin: auto; padding: 0; border: 1px solid var(--border); border-radius: 12px; background: var(--surface); color: var(--text); box-shadow: 0 20px 60px rgba(0,0,0,0.4); }
.su-dialog::backdrop { background: rgba(0,0,0,0.45); backdrop-filter: blur(2px); }
.su-dialog-header { display: flex; align-items: center; justify-content: space-between; padding: 14px 18px; border-bottom: 1px solid var(--border); }
.su-dialog-header h2 { font-size: 1rem; font-weight: 600; }
.su-dialog-close { width: 28px; height: 28px; padding: 0; display: flex; align-items: center; justify-content: center; border-radius: 50%; border: 1px solid var(--border); background: var(--surface); color: var(--text-muted); cursor: pointer; transition: all .15s; }
.su-dialog-close:hover { background: var(--surface-hover); color: var(--text); }
.su-dialog-body { padding: 16px 18px; font-size: 0.875rem; line-height: 1.55; color: var(--text); }
.su-dialog-body p { margin-bottom: 12px; }
.su-dialog-body p:last-child { margin-bottom: 0; }
.su-dialog-body code { font-family: var(--mono); background: var(--surface-hover); padding: 1px 5px; border-radius: 4px; font-size: 0.85em; }
.su-dialog-body a { color: var(--primary); text-decoration: none; }
.su-dialog-body a:hover { text-decoration: underline; }
.su-dialog-fineprint { font-size: 0.8rem; color: var(--text-muted); }
```

### Wiring (copy verbatim)

A standalone IIFE inside the tool's main `<script>` block, placed
**immediately after the theme-toggle IIFE** so it sits next to the other
chrome wiring:

```js
(() => {
    const dlg = document.getElementById('privacyDialog');
    document.getElementById('privacyOpen').addEventListener('click', () => dlg.showModal());
    document.getElementById('privacyClose').addEventListener('click', () => dlg.close());
})();
```

Three lines: open, close, and a reference. No state, no listeners on the
backdrop, no manual focus management - the native `<dialog>` handles all of
that. Tools that ship in IE-era syntax (`var` / `function ()`) like
`index.html` mirror the same style with `var` instead of `const`; the
behaviour is identical.

## Credits dialog (footer)

Every tool whose browser side runs any third-party JS carries a **Credits**
button next to **Privacy** in the footer fineprint. It opens a native
`<dialog>` listing the major open-source libraries that power the tool, each
linked to its GitHub repository. The dialog reuses the same `.su-dialog`
chrome (chrome, close button, backdrop, centering) as the Privacy dialog -
only the body content differs.

### Why a separate dialog instead of a section inside Privacy

Privacy is a legal-adjacent disclosure (what stays in the browser, what
Plausible collects); Credits is attribution / acknowledgement (which OSS
libraries the tool reuses). Conflating them buries each behind the other's
copy, and the open-source-attribution surface needs to grow per-tool while
the Privacy copy stays identical across every tool. Two dialogs keep each
one short and the per-tool diff small.

### When to omit the link

If a tool ships **zero** third-party browser JS (today: `index.html` and
`echo.html`), drop both the footer button and the `<dialog>` block - there
is nothing to credit. Vendored code that runs only inside the
`@su:responder-script` block (e.g. `echo.html`'s tiny-inflate) does not
count: the dialog scope is the browser-side experience the user actually
interacts with.

### Markup (copy verbatim, fill in the list)

Place as the **next sibling after `#privacyDialog`** (so the two dialogs sit
together at the end of `<body>`):

```html
<dialog id="creditsDialog" class="su-dialog" closedby="any" aria-labelledby="creditsDialogTitle">
    <header class="su-dialog-header">
        <h2 id="creditsDialogTitle">Credits</h2>
        <button type="button" class="su-dialog-close" id="creditsClose" aria-label="Close">
            <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 3l10 10M13 3L3 13"/></svg>
        </button>
    </header>
    <div class="su-dialog-body">
        <p>This tool is powered by the following open-source libraries:</p>
        <ul class="su-credits-list">
            <li><a href="https://github.com/&lt;owner&gt;/&lt;repo&gt;" target="_blank" rel="noopener noreferrer"><strong>name</strong></a> - short one-line description.</li>
        </ul>
        <p class="su-dialog-fineprint">All trademarks are property of their respective owners.</p>
    </div>
</dialog>
```

Content rules:

- One `<li>` per major library. Skip transitive deps - the goal is
  attribution of the libraries an informed reader would recognise, not
  exhaustive SBOM coverage.
- Link the library **name** (bolded with `<strong>`) to its canonical
  GitHub repository. No version numbers (the cdnjs / `data-su-bundle`
  pinned versions already record those; the dialog should not drift on
  every bump).
- After the link, a single hyphen surrounded by spaces, then a brief
  one-line description. **No em-dashes anywhere** - the visual hyphen is
  a single ASCII `-`.
- List order follows the order the libraries appear in `<head>` `<script>`
  tags (and after them, any libraries pulled in by `data-su-bundle` or
  dynamic `import()` calls).

### CSS (copy verbatim)

Place next to the existing `.su-dialog-body` rules:

```css
.su-credits-list { margin: 0 0 12px; padding-left: 20px; }
.su-credits-list li { margin-bottom: 6px; }
.su-credits-list li:last-child { margin-bottom: 0; }
```

### Wiring (copy verbatim)

A standalone IIFE inside the tool's main `<script>` block, placed
**immediately after the Privacy IIFE** so the two dialog wirings sit next to
each other:

```js
(() => {
    const dlg = document.getElementById('creditsDialog');
    document.getElementById('creditsOpen').addEventListener('click', () => dlg.showModal());
    document.getElementById('creditsClose').addEventListener('click', () => dlg.close());
})();
```

## Dialog backdrop dismissal (`closedby="any"`)

Every `.su-dialog` carries the HTML attribute `closedby="any"`. The native HTML
`<dialog>` element then closes on:

- the `Esc` key (native — always);
- the close button (wired explicitly in the Privacy / Credits IIFEs above);
- and a click on the backdrop (delivered by `closedby="any"`, Baseline Newly
  Available since 2025).

This removes the historical custom "click outside to dismiss" listener that older
versions of the chrome carried (`addEventListener('click', e => { if (e.target === dlg) dlg.close(); })`).
The browser also dispatches a synthetic `cancel` event when light-dismissed, which means
any future cleanup logic can listen for `cancel` instead of `click`-on-backdrop.

### Safari fallback (≤14 LOC)

Safari was the last engine to ship `closedby` and may still be missing it on some
in-support releases. The fallback below feature-detects `'closedBy' in HTMLDialogElement.prototype`
and only registers manual backdrop listeners when the property is absent. Place
it **immediately after the Credits IIFE** (or after the Privacy IIFE in tools
that ship no Credits dialog — `index.html`, `echo.html`):

```js
// Safari fallback for closedby="any" (Newly Baseline elsewhere; limited in Safari as of Q2 2026).
if (!('closedBy' in HTMLDialogElement.prototype)) {
    for (const dlg of document.querySelectorAll('dialog.su-dialog')) {
        dlg.addEventListener('click', (e) => {
            if (e.target !== dlg) return;
            const r = dlg.getBoundingClientRect();
            if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom) dlg.close();
        });
    }
}
```

The `e.target !== dlg` guard skips clicks that bubble from inside the dialog
content. The `getBoundingClientRect` check is required because the dialog's
hit-box covers the entire viewport when the backdrop is shown — the rectangle
comparison is what distinguishes "clicked the backdrop" from "clicked a child
form control whose own click handler stopped propagation". Tools that ship in
ES5-style chrome (`index.html`) mirror the same logic with `var` and IIFE
hoisting; the behaviour is identical and the LOC budget still holds.

Drop the fallback once Safari's Baseline status flips to Widely Available
(track via <https://webstatus.dev/features/dialog-closedby>); the `closedby="any"`
attribute on the markup is the source of truth.

## Tethered popovers (native `popover` + `commandfor`)

For dropdown menus, options popovers, and any other element that "tethers" to
an invoker button (Options gear, Export menu, OCR settings), use the native
**Popover API** (`popover` attribute) and **Invoker Commands** (`commandfor` +
`command="toggle-popover"`). The browser handles open/close, light-dismiss,
`Esc`, focus return, and top-layer z-stacking — no `stopPropagation`, no
document-level click listeners, no `aria-expanded` bookkeeping.

`popover` is Baseline Newly Available; Invoker Commands rolled out across all
three engines in late 2025. Both qualify under the Browser-support policy.

### Markup (copy verbatim)

```html
<div class="options-anchor">
    <button class="btn btn-sm" commandfor="optionsPopover" command="toggle-popover" aria-haspopup="true" title="Export options">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z"/></svg>
        <span class="opt-label">Options</span>
    </button>
    <div id="optionsPopover" popover class="options-popover" role="dialog" aria-label="Export options">
        …
    </div>
</div>
```

Notes:

- **Always use an inline SVG cog** (24×24 viewBox, rendered 14×14, stroke 1.8).
  The Unicode `&#9881;` gear glyph renders ~30 % smaller than a 14 px SVG icon
  inside the same `.btn-sm`, which makes it look "tiny" next to the other
  action icons (Copy / Export / Fullscreen are all stroke SVGs). The flex
  `gap: 5px` from `.btn` handles the icon↔label spacing — no `&nbsp;`.
- The invoker is **always** a `<button type="button">` (or a default-type
  button inside a form that does not submit). `commandfor` references the
  popover's `id`; `command="toggle-popover"` opens it if closed and vice
  versa. `show-popover` / `hide-popover` are also valid commands.
- Do **not** carry `aria-expanded` on the invoker. The user agent reflects
  expanded state into the accessibility tree automatically when the invoker
  is associated with a popover via `commandfor`. The literal HTML attribute
  is **only** maintained by the fallback below (so CSS that targets
  `[aria-expanded="true"]` for, e.g., chevron rotation must be paired with a
  `:has(…:popover-open)` selector — see CSS below).
- Keep `aria-haspopup="true"` (or `"menu"` for an action menu): it's still
  the most reliable signal across screen readers and Invoker Commands does
  not provide it.
- Keep the `.options-anchor` / `.export-anchor` wrapper for visual layout
  (it sits the button + popover together in the panel-actions flex row) and
  so CSS `:has()` selectors can scope to "this invoker is open".

### CSS (copy verbatim)

```css
.options-popover {
    /* [popover] starts in the top layer; the UA stylesheet applies
       `position: fixed; inset: 0; margin: auto;` (centered). Override `margin`
       to 0 and let JS pin the popover next to its invoker on the `toggle`
       event — see positionPopover() below. */
    margin: 0;
    max-width: min(360px, calc(100vw - 24px));
    min-width: 260px; padding: 14px 16px;
    background: var(--surface); border: 1px solid var(--border); border-radius: 10px;
    box-shadow: 0 10px 30px rgba(0,0,0,0.35);
}

/* Long unbroken text in checkbox descriptions (URLs, identifier examples)
   must wrap or the popover stretches across the viewport. */
.options-popover .opt-desc { overflow-wrap: anywhere; }

/* Menu-style popover (column of buttons). Layout the *items*, not the
   popover, with full-width block + margin spacing. */
.export-menu { margin: 0; min-width: 110px; max-width: calc(100vw - 24px); padding: 6px; background: var(--surface); border: 1px solid var(--border); border-radius: 8px; box-shadow: 0 10px 30px rgba(0,0,0,0.35); }
.export-item { display: block; width: 100%; padding: 7px 14px; ... }
.export-item + .export-item { margin-top: 2px; }

/* Chevron / caret rotation when the menu is open:
   :popover-open wins in the native path; [aria-expanded] covers the no-popover JS fallback. */
.export-anchor:has(.export-menu:popover-open) .chevron,
#exportBtn[aria-expanded="true"] .chevron { transform: rotate(180deg); }
```

**Cascade-origin trap: never set `display:` on the popover itself.** The UA
stylesheet hides closed popovers with
`[popover]:not(:popover-open) { display: none; }`. Author-origin rules beat
UA-origin rules at *any* specificity — so an author `.export-menu { display:
flex; }` will keep the popover permanently visible (and, paired with our
`margin: 0` override, stuck at the `inset: 0` top-left corner because the JS
tether only fires on open). Lay out menu items with per-item `display: block`
+ margin (above), or use a flex/grid container *inside* the popover. Other
display-changing properties (`display: grid`, `display: contents`,
`display: block !important`) all hit the same trap.

Do **not** set `z-index` on the popover. `[popover]` shown elements are
promoted to the top layer; they sit above any positioned siblings without
any z-index dance and ignore `overflow: hidden` on their containing scroll
parent. This is the main reason to migrate.

**Why not CSS Anchor Positioning (`position-area`)?** It would replace the
JS helper below with a one-liner, but as of Q2 2026 anchor positioning is
still Chromium-only (Firefox / Safari have partial or flagged support). It
is **not** Newly Baseline, so it fails the Browser-support policy. Once it
flips to Baseline, the JS helper can be deleted and `position-area: bottom
span-left; margin-top: 8px;` reintroduced.

### JavaScript helpers

Three helpers per tool: a uniform `hidePopoverEl()` for close paths, a
viewport-coords `positionPopover()` for tethering, and a `wireTether()`
that attaches both a `click` listener (synchronous, pre-open) and a
`beforetoggle` listener (catches keyboard-driven opens) per invoker /
popover pair. The same call site works in both the native and fallback
paths because the fallback IIFE dispatches a synthetic `toggle` event.

```js
// Close any [popover] element across the native + fallback code paths.
const hidePopoverEl = (el) => {
    if (typeof el.hidePopover === 'function') { if (el.matches(':popover-open')) el.hidePopover(); }
    else el.hidden = true;
};

// Pin an open [popover] right-aligned just below its invoker. No-op in the
// fallback path: the fallback IIFE already positions the popover via inline
// `position: absolute; top: calc(100% + 6px); right: 0;` styles, so guarding
// on `showPopover` (absent in browsers without Popover API) lets the same
// `beforetoggle` listener be wired unconditionally.
const positionPopover = (popover, invoker) => {
    if (typeof popover.showPopover !== 'function') return;
    const r = invoker.getBoundingClientRect();
    popover.style.position = 'fixed';
    popover.style.top = `${Math.round(r.bottom + 6)}px`;
    popover.style.left = 'auto';
    popover.style.right = `${Math.round(window.innerWidth - r.right)}px`;
    popover.style.bottom = 'auto';
};

// Wire both a `click` listener (synchronous, pre-open) and a `beforetoggle`
// listener (catches programmatic showPopover() and keyboard-driven opens).
// `click` is the primary path — it fires *before* the browser performs the
// [commandfor] default action (showPopover), so the inline `top`/`right` are
// already on the element by the time it enters the top layer.
const wireTether = (invoker, popover) => {
    invoker.addEventListener('click', () => positionPopover(popover, invoker));
    popover.addEventListener('beforetoggle', (e) => { if (e.newState === 'open') positionPopover(popover, invoker); });
};
wireTether(els.optionsBtn, els.optionsPopover);
wireTether(els.exportBtn, els.exportMenu);

// `toggle` is the right place for post-open side-effects (refreshing menu
// items, tearing down language suggestions, etc.) where a one-frame delay
// is fine.
els.exportMenu.addEventListener('toggle', (e) => {
    if (e.newState === 'open') syncMenuItems();
});
```

**Why both `click` and `beforetoggle`?** Some browsers (and some Chromium
versions when the popover is opened through Invoker Commands) fire
`beforetoggle` after the popover has already been laid out at the UA
default `inset: 0` location, producing a visible top-left flash before
the JS can reposition. The `click` listener on the invoker runs **inside
the dispatch of the click event**, which is strictly before the browser
performs the invoker's default action (showPopover) — so the inline
position styles set there are already on the element when the popover
enters the top layer. `beforetoggle` then covers paths that bypass a
mouse click on the invoker (e.g. a `.showPopover()` from elsewhere, or
keyboard activation that some browsers route differently).

`beforetoggle` and `toggle` are the Popover API's lifecycle hooks. Both
carry `e.newState` / `e.oldState` strings of `"open"` / `"closed"`. The
fallback IIFE below dispatches a `toggle` event manually (with a matching
`newState` property) but does **not** dispatch `beforetoggle` — it
doesn't need to because the fallback path positions the popover with
inline styles directly inside the click handler before showing it.

The positioning runs **on every open** rather than just once, so the
popover follows the invoker if the user has scrolled the panel-actions row
sideways before re-opening. Resize listeners are deliberately not wired —
the popover's light-dismiss model means a viewport resize that hides the
invoker also closes the popover.

### Fallback IIFE (≤14 LOC; copy verbatim)

Place it **once per tool**, immediately after the per-popover
`toggle`-event wiring. It feature-detects `'popover' in HTMLElement.prototype`
and `'commandForElement' in HTMLButtonElement.prototype` and, on miss,
wires every `[popover]` + `[commandfor]` pair in the document with the
old "click invoker / click outside / Esc to close" behaviour:

```js
// Fallback for engines without Popover API + Invoker Commands (older Safari, older Firefox).
if (!('popover' in HTMLElement.prototype) || !('commandForElement' in HTMLButtonElement.prototype)) {
    const pops = document.querySelectorAll('[popover]');
    for (const p of pops) { p.hidden = true; p.style.position = 'absolute'; p.style.top = 'calc(100% + 6px)'; p.style.right = '0'; }
    const setState = (p, open) => { if (p.hidden === !open) return; p.hidden = !open; p.dispatchEvent(Object.assign(new Event('toggle'), { newState: open ? 'open' : 'closed' })); };
    const closeAll = () => { for (const p of pops) setState(p, false); for (const i of document.querySelectorAll('[commandfor]')) i.setAttribute('aria-expanded', 'false'); };
    for (const inv of document.querySelectorAll('[commandfor]')) {
        const t = document.getElementById(inv.getAttribute('commandfor'));
        if (!t) continue;
        inv.addEventListener('click', (e) => { if (inv.disabled) return; e.stopPropagation(); const opening = t.hidden; closeAll(); if (opening) { setState(t, true); inv.setAttribute('aria-expanded', 'true'); } });
    }
    document.addEventListener('click', (e) => { for (const p of pops) if (!p.hidden && !p.contains(e.target)) closeAll(); });
    document.addEventListener('keydown', (e) => { if (e.key === 'Escape') closeAll(); });
}
```

Behavioural notes:

- The IIFE is **idempotent across popovers in the same document**: it picks
  up every `[popover]` and every `[commandfor]` in one pass, so adding a new
  popover to a tool needs zero new fallback wiring.
- `inv.disabled` is honoured (the Export button starts disabled). The
  short-circuit is required because clicks on a disabled button still
  bubble in the fallback path.
- The fallback sets the literal `aria-expanded` attribute (the native path
  does not). The chevron-rotation CSS above includes both selectors so the
  visual matches in either path.
- The fallback dispatches a synthetic `toggle` event with `newState` so
  call-site listeners (the OCR popover's language-suggestions teardown,
  the Export menu's `syncMenuItems()` resync) work unchanged.
- The fallback re-applies the historical `position: absolute; top:
  calc(100% + 6px); right: 0;` rules via inline `style` so the popover
  still tethers to the invoker without `position-area` support.

### Reference implementations

- [`dev/tools/md.html`](md.html) — HTML options gear
  popover + Export dropdown menu (the canonical pair).
- [`dev/tools/pdf-extractor.html`](pdf-extractor.html) — OCR settings popover
  (uses the `toggle` event for language-suggestions teardown) + Export menu
  (uses `toggle` for the per-open `syncExportMenuItems()` resync).

Drop the fallback IIFE once `'commandForElement' in HTMLButtonElement.prototype`
reaches Widely Available on the Baseline tracker. The HTML markup and the
two helpers stay; everything in the `if (!('popover' …))` block becomes dead
code that the minifier strips.

## Form validation feedback (`:user-invalid` + `aria-invalid`)

Inputs with `required`, `min`/`max`, `type="number"`, `type="email"`, or
`pattern="…"` constraints must use **`:user-invalid`** for their red-border
styling — never `:invalid`. `:invalid` fires the moment the page renders, so
inputs hydrated from URL state (`echo.html`'s `status` field briefly fails
`min=100/max=599` during parsing) or empty `required` fields on first load
flash an error before the user has touched anything. `:user-invalid` defers
until the user has either edited the field or attempted to submit — matching
the way browsers themselves time native validation tooltips.

Baseline Newly Available since 2024-12. No fallback is needed: browsers
without `:user-invalid` simply never apply the rule, so the input gets no
custom red border — strictly better than the historical `:invalid`-on-load
flash.

### CSS (copy verbatim, adjust selectors to the tool's input classes)

```css
.input:user-invalid, .textarea:user-invalid, .input-mono:user-invalid {
    border-color: var(--danger);
    box-shadow: 0 0 0 1px var(--danger);
}
.form-input:user-invalid { border-color: var(--danger); }
```

Add a `--danger` color variable to **both** theme blocks if it isn't already
there (the rest of the palette lives in the existing `:root` /
`[data-theme="light"]` rules). The borealis dark danger is `#dc4a44`; the
light counterpart is `#bd271e`:

```css
:root, [data-theme="dark"] { … --danger: #dc4a44; … }
[data-theme="light"]       { … --danger: #bd271e; … }
```

### Accessibility sync (`aria-invalid`)

CSS handles the visual; assistive tech still needs the literal `aria-invalid`
attribute to announce the same state. Set it on `blur` (after the user has
moved away from the field, matching the `:user-invalid` trigger) rather than
on every keystroke:

```js
for (const i of document.querySelectorAll('input[required], input[type=number], input[type=email]')) {
    i.addEventListener('blur', () => i.setAttribute('aria-invalid', String(!i.matches(':valid'))));
}
```

`:valid` is the inverse of `:invalid` and works on every browser the tools
target. The string-coerced boolean (`"true"` / `"false"`) is the
`aria-invalid` contract (it is **not** boolean ARIA, so the literal string
matters). Place this loop just after the dialog-fallback IIFE so it runs
once the DOM is fully wired.

### Reference implementations

- [`dev/tools/echo.html`](echo.html) — status-code number input + header value
  inputs; URL-hydrated state.
- [`dev/tools/mock-saml-idp.html`](mock-saml-idp.html) — required username +
  email form.

## Responsive (mobile)

```css
@media (max-width: 640px) {
    header { padding: 0 12px; }
    .logo-svg { height: 20px; }
    .logo-badge { font-size: 11px; padding: 2px 7px; }
    .btn { padding: 6px 10px; font-size: 12px; }
}
```

### Reduced motion (`prefers-reduced-motion: reduce`)

Every tool ships a single four-line override that flattens animations and
transitions when the OS-level motion preference is set. Sits alongside the
"Typography polish" rules added in the section above, and ships in every tool
verbatim:

```css
@media (prefers-reduced-motion: reduce) {
    *, *::before, *::after {
        animation-duration: 0.01ms !important;
        animation-iteration-count: 1 !important;
        transition-duration: 0.01ms !important;
        scroll-behavior: auto !important;
    }
}
```

Rules:

- The selector is the universal `*`; `!important` is required to override
  per-element `transition: … .25s` rules in the shared chrome (theme
  toggle, header surface, panel header, button hovers) without modifying
  each one individually.
- `0.01ms` (not `0s`) so the `transitionend` / `animationend` events still
  fire — required by EUI-style components that hide themselves on the
  transition-end event (e.g. the toast notification).
- `scroll-behavior: auto !important` cancels any smooth-scroll set via
  `scroll-behavior: smooth` in third-party libraries (`marked`-rendered
  anchor links, `pagedjs` page transitions, etc.).
- No fallback is needed; browsers without the media query simply never
  match it. Baseline since 2020.

## Resource priority (`fetchpriority`)

Heavy third-party libraries are demoted with **`defer` + `fetchpriority="low"`**
so the browser fetches them after the LCP-critical resources (Google Fonts
CSS, inline CSS, header logo) and the inline init shims have had a chance to
run. Applies to:

| Library                   | Tools                      | Notes                                                                                                                  |
|---------------------------|----------------------------|------------------------------------------------------------------------------------------------------------------------|
| `jose`                    | `jwt-debugger.html`        | Only used in async sign/verify handlers — `defer` is safe.                                                             |
| `forge`                   | `certificate-decoder.html` | Only used in PEM decode handlers — `defer` is safe.                                                                    |
| `jsrsasign` + `pako`      | `mock-saml-idp.html`       | Used inside `DOMContentLoaded` setup and click handlers — `defer` is safe.                                             |
| `highlight.js` + `pako`   | `saml-decoder.html`        | Used in `buildStaticXmlView` and inflate handlers — `defer` is safe.                                                   |
| `marked` + `highlight.js` | `md.html`                  | `fetchpriority="low"` **only** — `defer` cannot be added because the inline script calls `marked.use(…)` at top-level. |
| Plausible loader          | every tool                 | Already `defer`; add `fetchpriority="low"` next to it.                                                                 |

Canonical snippet:

```html
<script src="https://cdnjs.cloudflare.com/ajax/libs/jose/4.14.4/index.umd.min.js" defer fetchpriority="low"></script>
```

When in doubt, audit every reference to the library: any call at the
top-level of a `<script>` (no `function`, `() =>`, or event listener
wrapping it) blocks adding `defer`. Either restructure the call to live
inside a `DOMContentLoaded` listener, or stop at `fetchpriority="low"`.

`fetchpriority` is Baseline since 2024-09. Browsers without support ignore
the attribute — no behaviour change.

## JavaScript Style

These tools target evergreen browsers (current Chrome / Firefox / Safari / Edge) - no
transpilation, no polyfills, no IE / legacy-browser support. Write modern JavaScript and
keep the embedded `<script>` blocks compact and idiomatic.

**Required:**

- **`const` by default, `let` only when reassigned.** Never use `var`.
- **Arrow functions** for callbacks and short helpers. Use named `function` declarations
  only for top-level helpers where the name aids readability or the function needs
  hoisting.
- **Template literals** for any string with interpolation, multi-line content, or HTML
  fragments. Never build HTML / CSS strings via `+` concatenation or `[…].join('\n')`.
- **`for…of`** over `Array.prototype.forEach` for plain iteration.
- **Spread syntax** to convert `NodeList` / iterables to arrays:
  `[...el.querySelectorAll('…')]`.
- **Optional chaining (`?.`) and nullish coalescing (`??`)** instead of `&&`/`||` chains
  when the intent is "value or fallback when null/undefined".
- **`async`/`await`** for clipboard, fetch, and any other promise-returning APIs. Avoid
  raw `.then()` chains unless the call site can't be `async`.
- **Destructuring** for object/array unpacking when it improves readability.
- **`catch {}`** (no unused binding) when the error is intentionally ignored - never
  `catch (e) {}` with an unused `e`.
- **Hoist constants** (CDN URLs, regexes, SVG markup, repeated HTML fragments) to
  module-top `const`s instead of inlining them at every use site.
- **Cache element references** in a single object rather than calling
  `document.getElementById` repeatedly; a tiny `const $ = (id) => document.getElementById(id);`
  helper plus a frozen `els = { … }` map keeps things tidy.

**Avoid:**

- `var` - `const`/`let` are the only acceptable bindings.
- `function () {}` callbacks - use arrow functions.
- String concatenation with `+` for HTML / CSS / multi-line text.
- Manual `Array.from(nodeList)` - use `[...nodeList]`.
- Truthy/falsy `&&`/`||` for null-fallbacks where `??` is the correct operator.
- `e` in `catch (e) {}` when unused - drop the binding.

**Optional but encouraged:**

- Top-level `await` is fine inside an `async` IIFE if the script needs it.
- Promise-wrap legacy event-driven APIs (e.g. `iframe.onload`, paged.js polling) so the
  control flow reads top-to-bottom.
- Use private object short-hand (`{ foo, bar }`) and computed property names where they
  make code clearer.

The reference implementation in `md.html` follows all of the above and is
the canonical example. When modifying an existing tool that still uses legacy syntax,
modernize the surrounding code in the same edit.

### Long tasks (`scheduler.yield()` + `content-visibility: auto`)

Any synchronous loop that processes >50 ms of work is a long task. It blocks
input handlers, kills Interaction-to-Next-Paint (INP), and on the tools
the user-perceived symptom is "the page froze mid-render". Two
complementary tools:

#### `scheduler.yield()` for chunked work

`scheduler.yield()` returns a Promise that resolves after the browser has
had a chance to handle other work (input, paint, microtasks). Unlike
`setTimeout(0)`, the continuation **stays in the same priority queue**, so
yielding mid-loop doesn't lose ordering vs. concurrent click handlers.
Baseline since 2025 in Chromium + Firefox; Safari is still missing it, so
fall back to either `setTimeout(0)` (event-loop yield, no paint guarantee)
or `requestAnimationFrame` (yields and waits for paint — better when the
loop just appended a DOM node that needs to render).

Canonical helper (copy verbatim):

```js
// scheduler.yield → setTimeout(0) for pure event-loop yielding.
const yieldToMain = () => 'scheduler' in window && typeof scheduler.yield === 'function'
    ? scheduler.yield()
    : new Promise((r) => setTimeout(r, 0));

// scheduler.yield → requestAnimationFrame when the loop needs a paint between chunks.
const yieldToMain = () => 'scheduler' in window && typeof scheduler.yield === 'function'
    ? scheduler.yield()
    : new Promise((r) => requestAnimationFrame(() => r()));
```

Pick the fallback to match what the loop needs (event-loop slice vs paint).
The Markdown renderer in `md.html` uses the first form
(highlight blocks are cheap and don't need a paint between them); the PDF
extractor's `renderShots` loop uses the second (each iteration appends an
`<img>` whose paint the user sees as progress).

Canonical 50 ms deadline pattern:

```js
let deadline = performance.now() + 50;
for (const block of items) {
    doWork(block);
    if (performance.now() >= deadline) {
        await yieldToMain();
        deadline = performance.now() + 50;
    }
}
```

50 ms is the INP "long task" boundary — work below that doesn't trigger
Chrome's `longtask` PerformanceObserver entry and doesn't show up in
WebPageTest's Total Blocking Time. Don't yield more often than that: every
yield costs a microtask + (optionally) a paint, and a too-aggressive
deadline cuts throughput without improving INP.

#### `content-visibility: auto` for long lists

Any list of >20 items (PDF page captures, outline rows, search results,
log entries) should carry `content-visibility: auto` plus a
`contain-intrinsic-size` hint. The browser then skips painting and hit-
testing for off-screen items, with the hint reserving scroll space so the
scrollbar doesn't jump as items materialise.

```css
.shots-figure       { content-visibility: auto; contain-intrinsic-size: auto 800px; }
.outline-list .outline-list { content-visibility: auto; contain-intrinsic-size: auto 200px; }
```

Widely Baseline; no fallback needed (older browsers ignore the property,
content paints as before). The intrinsic-size value should match the
typical rendered height of the element — too small causes scrollbar jumps
when items hydrate, too large wastes scroll real estate. Eyeball the
median height once and pin it.

#### Reference implementations

- [`dev/tools/md.html`](md.html) `render()` —
  yields every 50 ms between hljs `highlightElement` calls. The 150 ms
  `updatePreview` debounce hides the async hop from the user.
- [`dev/tools/pdf-extractor.html`](pdf-extractor.html) `renderShots()` and
  `parsePdf()` — uses `yieldToMain()` (`scheduler.yield` → rAF fallback)
  so progress updates and freshly-appended page captures paint before the
  next chunk of synchronous work runs.

### Busy-state gating (long async pipelines)

A long-running async pipeline (PDF parse, multi-second OCR, generation
step) must put the whole UI into a busy state for the duration. Three
hazards motivate this:

- **Re-entrancy** — mid-flight clicks on the Clear / Replace / Parse
  buttons restart the engine or detach an input buffer that the running
  pipeline still owns (e.g. pdf.js transfers the ArrayBuffer to its
  worker, so a second click on the same `pdfFile.bytes` view dispatches a
  detached buffer).
- **Stale state exposure** — switching view tabs while the result panel
  shows `<progress>` exposes the *previous* parse's text/JSON/markdown,
  which the user reasonably interprets as the new result.
- **No-op interactions** — toggling OCR options or upload affordances
  during a parse has no effect (the engine already snapshotted them) but
  looks like it does, which is worse than disabling them.

The canonical shape (see `setParsingBusy()` in
[`dev/tools/pdf-extractor.html`](pdf-extractor.html)):

```js
function setParsingBusy(busy) {
    // Always-gated controls: re-derive their non-busy enabled state from
    // app-state predicates, not from a snapshot taken on entry. The
    // pipeline itself flips Share/Copy/Export via setExportEnabled() inside
    // the try block, and a snapshot-restore would clobber that work.
    els.clearBtn.disabled = busy || !pdfFile;
    els.replaceBtn.disabled = busy || !pdfFile;
    els.ocrBtn.disabled = busy;
    for (const t of [els.tabText, els.tabJson, /* … */]) t.disabled = busy;

    // Output-side controls (Share/Copy/Export) are owned by setExportEnabled.
    // Force-disable on entry; leave alone on exit so the pipeline's own
    // setExportEnabled(true|false, …) call wins.
    if (busy) { els.shareBtn.disabled = true; els.copyBtn.disabled = true; els.exportBtn.disabled = true; }

    // Dropzone / large click-target areas: combine `pointer-events: none`
    // (blocks click + drag) with `aria-busy="true"` (screen-reader signal)
    // and `tabindex="-1"` (out of focus order, so keyboard Enter can't
    // re-trigger the file picker).
    els.dropzone.classList.toggle('is-busy', busy);
    if (busy) els.dropzone.setAttribute('aria-busy', 'true');
    else els.dropzone.removeAttribute('aria-busy');
    els.dropzone.tabIndex = busy ? -1 : 0;
}
```

```css
.dropzone.is-busy { pointer-events: none; cursor: default; opacity: .85; }
.dropzone.is-busy.dragover { border-color: var(--border); background: var(--surface); }
```

Call `setParsingBusy(true)` at the **top** of the async function (before
any `await`) and `setParsingBusy(false)` in a `finally` so a thrown
exception still releases the lock. The Parse-button restoration in the
`finally` is separate (`els.parseBtn.disabled = !pdfFile`) because their
disabled state mirrors file-loaded state, not the snapshot.

## Live updates / polling (visibility-aware)

Any tool that polls or long-polls a backend (repeated `fetch`, `EventSource`, or a
long-poll loop like the webhook inspector's `secutils.kv.watch`) **MUST**:

- **Abort the in-flight request and stop scheduling new ones when the tab is hidden**
  (`visibilitychange` -> `document.hidden`) and on `pagehide` (navigation / close / bfcache).
- **Re-establish from the last cursor when the tab becomes visible again.**

Rationale: a backgrounded or closed tab that keeps polling wastes server work for output
nobody is looking at. For a long-poll it is worse - each open request parks a **server-side
V8 isolate (+ worker thread)** for the whole window, and with the safe actix default
(`h1_allow_half_closed = true`) the server **cannot** detect the client disconnect on its own
(it only notices on its next socket write, which a parked long-poll never does). So the client
is the only party that can promptly close the connection - it must do so itself.

Model the loop as two pieces of state: **intent** (the user toggled it on; drives the toggle
pill) and **running** (a request loop is actually open). Pausing for visibility clears
*running* but preserves *intent*, so it resumes transparently.

Canonical shape (reference implementation: [`dev/tools/webhook.html`](webhook.html)):

```js
let abort = null;       // AbortController for the in-flight request
let running = false;    // a request loop is actually open
let enabled = false;    // user intent (drives the toggle UI)

// Record intent + reflect it in the UI, but only open a connection while visible.
function start() {
    enabled = true;
    // ...reflect enabled in the toggle UI here...
    if (!document.hidden) runLoop();
}
function runLoop() {
    if (running) return;            // idempotent
    running = true;
    abort = new AbortController();
    (async () => {
        while (running /* && still-current */) {
            try {
                const res = await fetch(url, { signal: abort.signal });
                /* ...handle response, advance cursor... */
            } catch (e) {
                if (e.name === 'AbortError') break;
                await sleep(2000);
            }
        }
    })();
}
// Pause WITHOUT clearing intent, so visibility can resume it from the last cursor.
function pauseLoop() {
    running = false;
    if (abort) { try { abort.abort(); } catch {} abort = null; }
}
// Explicit user-off: clears intent + UI, then pauses.
function stop() {
    enabled = false;
    // ...reflect disabled in the toggle UI here...
    pauseLoop();
}

document.addEventListener('visibilitychange', () => {
    if (document.hidden) { if (running) pauseLoop(); }
    else if (enabled && !running) runLoop();
});
window.addEventListener('pagehide', () => { if (running) pauseLoop(); });
```

**Agent instruction:** when building a new tool, or adding any polling / long-polling to an
existing one, implement this pattern and **explicitly confirm with the developer** that the
visibility-abort + resume wiring is in place before considering the tool done.

## Pre-deploy verification

After editing any tool, run these checks before `make deploy-tools`. They take seconds,
catch the failure modes the deploy pipeline can't (the deploy itself just minifies and
PUTs - it does not parse the JS or smoke-test it), and don't need any browser.

### 1. Inline `<script>` syntax check

Parse every inline script block of every modified file with `node:vm`. This catches
typos, missing brackets, accidental top-level `await` outside an `async` context, etc. -
all the things `node --check` catches for `.js` files but with the inline-script
extraction handled.

```bash
node -e "
const fs = require('node:fs');
const vm = require('node:vm');
const files = process.argv.slice(1);
for (const f of files) {
  const html = fs.readFileSync(f, 'utf8');
  const re = /<script((?:[^>])*)>([\s\S]*?)<\/script>/g;
  let m, idx = 0, allOk = true;
  while ((m = re.exec(html))) {
    const attrs = m[1], code = m[2];
    if (!code.trim()) { idx++; continue; }
    if (/src\s*=/.test(attrs)) { idx++; continue; }
    // JSON-LD blocks ship as <script type=\"application/ld+json\">; they are
    // valid JSON, not JavaScript, so node:vm would reject the leading '{'.
    if (/type\s*=\s*[\"']application\/ld\+json[\"']/i.test(attrs)) { idx++; continue; }
    // data-su-bundle placeholders ship as <script type=\"text/plain\"> and are
    // filled at deploy time by the Vite-built bundle; nothing for node:vm to
    // syntax-check here -- the sub-package's own toolchain validates it.
    if (/type\s*=\s*[\"']text\/plain[\"']/i.test(attrs)) { idx++; continue; }
    try { new vm.Script(code, { filename: \`\${f}#\${idx}\` }); }
    catch (e) { console.log('FAIL', f, 'script #' + idx, '->', e.message); allOk = false; }
    idx++;
  }
  console.log(allOk ? 'OK  ' : 'FAIL', f, '(', idx, 'script tags )');
}
" -- dev/tools/echo.html dev/tools/jwt-debugger.html
```

### 2. Minifier dry-run

Run `html-minifier-terser` with the exact options [`deploy.ts`](deploy.ts) uses. This
catches issues that only surface after minification (e.g. `minifyJS` failing on a stray
syntax error, or `removeComments` accidentally stripping something load-bearing).

```bash
cd dev/tools && node --input-type=module -e "
import { readFileSync } from 'node:fs';
import { minify } from 'html-minifier-terser';
for (const f of process.argv.slice(1)) {
  const html = readFileSync(f, 'utf8');
  try {
    const out = await minify(html, {
      collapseWhitespace: true, removeComments: true, minifyCSS: true, minifyJS: true,
      removeRedundantAttributes: true, removeScriptTypeAttributes: true,
      removeStyleLinkTypeAttributes: true,
    });
    console.log('OK  ', f, '->', out.length, 'bytes', '(', html.length, 'src )');
  } catch (e) { console.log('FAIL', f, '->', e.message); }
}
" -- echo.html jwt-debugger.html
```

### 3. URL-state round-trip

If the tool stores state in the URL (see "URL state encoding"), confirm the helpers
round-trip. Easiest to inline a copy of `encodeState` / `decodeState` and feed it
representative payloads (small, large, unicode, the tool's own default state). Anything
that doesn't satisfy `decodeState(await encodeState(input)) === input` is a bug.

### 4. Responder-script smoke test (only for tools with `@su:responder-script`)

Extract the `@su:responder-script` block, run it under `node:vm` with a stub `Deno.core`
context, and verify it returns the expected response shape for: a valid input, a
malformed input (must return a clean error response, not throw or hang), and a
missing-input case (must `return null` so the static body is served). See `echo.html`
for the wire-format pairing pattern.

These four checks are what should pass before `make deploy-tools` (or before opening a
PR if a deploy isn't immediate). Live verification in the browser still belongs in the
post-deploy step.

### 5. Cross-cutting checks for new / renamed tools

When you add a tool (or change its alias), the four script-level checks above are not
enough - there's also static metadata that has to stay in sync across two repos. Run:

```bash
make tools-check                       # asserts promo home + README + skill .md presence
node --check scripts/tools-check.ts    # script self-check (parses the script itself)
make tools-og                          # regenerates OG images from the registry
make e2e-tools-test                    # exercises SEO + skill .md against the live host
```

`make tools-check` walks every `dev/tools/*.html`, reads `<meta name="su-tool-promote">`,
and asserts:

1. Every promoted tool is linked from the marketing home page hero strip / cards.
2. Every promoted tool is listed in the root `README.md` "Free single-page tools" table.
3. Every promoted tool has a sibling `<name>.skill.md` file.
4. No non-promoted tool leaks into the marketing home page (the index page itself
   is exempt - it is always linked via "Browse all tools").

`make e2e-tools-test` runs the per-tool Playwright specs against the live tools host
(`BASE_URL` defaults to `https://tools.secutils.dev`). Each spec asserts the SEO head
block, the skill .md is reachable as `text/markdown`, and the tool's primary functional
flow works end-to-end.

## New-tool checklist

A condensed end-to-end checklist for adding a tool. Each step references the section
that explains it in detail.

1. **Author the HTML** - single file under `dev/tools/<name>.html`, header with logo,
   skill link button (see "Skill link button"), and theme toggle; body styled with the
   shared brand variables; full SEO head block (see "SEO requirements"); `<noscript>`
   fallback; `su-tool-path`, `su-tool-name`, `su-tool-description`,
   `su-tool-promote` meta tags; the Plausible analytics snippet in `<head>` (see
   "Analytics (Plausible)"); **bottom "more free tools" banner** as the last child of
   `<main>` (see    "More free tools bottom CTA"); footer with a **Privacy** button (see
   "Footer") backed by the canonical `<dialog>` block as the last child of `<body>`
   (see "Privacy dialog"); if the tool ships any third-party browser-runtime JS,
   a sibling **Credits** button next to Privacy and a matching `<dialog>` listing
   each library with a GitHub link (see "Credits dialog"). Do NOT add a separate
   `<nav class="su-related">` block - the banner is the sole related-tools surface.
2. **Author the skill** - `dev/tools/<name>.skill.md` as a real Claude Code / Cursor
   SKILL.md (terse `name` + `description` frontmatter, rich Markdown body with `## Inputs`,
   `## Wire format`, `## How to produce the URL` runnable snippet, `## After producing`,
   `## Caveats`). Mirror the shape of `echo.skill.md` - see "AI-agent surface".
3. **Add the tool to the registry** - append a row to `e2e/tools/registry.ts`
   with the tool's OG/E2E-specific metadata only (accent, icon, applicationCategory).
   Name / path / description / promotion live in the HTML `<meta>` tags and are
   imported automatically.
4. **Add the tool to `index.html`** - new card in the `.tool-list` container.
5. **Pre-create the responder IDs** - two responders (one HTML, one MD) on the
   responders backend; capture both IDs into `.env` as
   `SECUTILS_HTML_APP_RESPONDER_ID_<TOOL>` and `SECUTILS_HTML_APP_RESPONDER_ID_<TOOL>_MD`.
   The cross-cutting agent-discovery aggregate IDs (`_LLMS_TXT`, `_ROBOTS_TXT`,
   `_SITEMAP_XML`, `_AGENT_SKILLS_INDEX`) are one-time per environment and re-used
   for every tool deploy.
6. **Run the pre-deploy checks** - inline-script syntax (`#1`),
   `html-minifier-terser` dry-run (`#2`), URL-state round-trip (`#3` if applicable),
   responder-script smoke (`#4` if applicable).
7. **Generate OG images** - `make tools-og` (writes `og-<slug>.png` and
   `og-<slug>-light.png`).
8. **If `promote: true`**: add a card to the marketing site's home `#free-tools`
   section (the bottom card list - there is no longer a hero chip strip) and add a
   row to the README "Free single-page tools" table. **You do NOT need to touch
   `sitemap.xml`, `robots.txt`, `llms.txt`, or `agent-skills/index.json`** - those
   are regenerated from the HTML registry on every `make deploy-tools` run.
9. **Add an E2E spec** - `e2e/tools/<slug>.spec.ts` based on
   [`e2e/tools/jwt.spec.ts`](../../e2e/tools/jwt.spec.ts).
10. **Verify cross-cutting**: `make tools-check`, `make e2e-tools-test`.
11. **Deploy** - `make deploy-tools` (deploys the HTML, the `.skill.md`, and
    refreshes the four agent-discovery aggregates). The aggregate refresh requires
    `_LLMS_TXT`, `_ROBOTS_TXT`, `_SITEMAP_XML`, and `_AGENT_SKILLS_INDEX` env vars
    to be set; missing IDs produce yellow `⚠ skipped` warnings rather than failing
    the deploy.

## PDF Export (optional)

Tools that produce printable artifacts (rendered articles, decoded certificates, JWT
breakdowns, etc.) can offer a PDF export without breaking the single-page-HTML constraint.
The pattern, demonstrated in `md.html`, is:

1. Add a `↓ PDF` action button next to the existing download / copy actions.
2. Build a self-contained printable HTML document (same brand fonts and palette as the
   on-screen preview) with the article content and a small `<script>` tag that lazy-loads
   [Paged.js](https://pagedjs.org) from a CDN. Paged.js paginates the document into A4
   pages using CSS Paged Media (`@page`, `@bottom-center`, `string-set`, etc.) - vector
   text, selectable, fully styled to match the preview.
3. Inject that document into a hidden, off-screen `<iframe>` via `srcdoc`.
4. Inside the iframe, listen for `pagedjs:rendered` on `window` and flip a `__suPdfReady`
   flag; the parent polls that flag, then calls `iframe.contentWindow.print()`. The
   browser's native "Save as PDF" produces a vector PDF that is byte-for-byte consistent
   with the preview.
5. Clean up the iframe on `afterprint` (with a 30 s safety timeout for browsers that don't
   fire it reliably).

No WASM, no server round-trip, ~150 KB CDN script loaded only on first export. Always force
`data-theme="light"` on the print document - yellow-on-dark is great on screen but reads
poorly on paper.
