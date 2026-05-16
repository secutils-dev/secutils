# Secutils.dev Single-Page Tool Apps - Style Guide

All tools in `dev/tools/` are standalone single-HTML-file apps (embedded CSS + JS) styled to look consistent with the Secutils.dev web application. Use `markdown-to-html.html` as the canonical reference implementation.

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
- **Composes with the auto-injected Markdown-negotiation prelude.** `deploy.ts` always
  wraps every HTML responder's script (whether opt-in or empty) with a ~250 B prelude
  that 302-redirects `Accept: text/markdown` requests to the `<slug>.md` sibling. Your
  `@su:responder-script` body becomes the inner expression and runs only when the
  prelude does not redirect. See **"Markdown content negotiation"** below.

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

| Surface           | URL                                          | Content type     | Source on disk                          |
|-------------------|----------------------------------------------|------------------|-----------------------------------------|
| Tool page         | `https://{{TOOLS_HOST}}/<path>`              | `text/html`      | `dev/tools/<name>.html`                 |
| Per-tool skill    | `https://{{TOOLS_HOST}}/<path>.md`           | `text/markdown`  | `dev/tools/<name>.skill.md`             |
| Aggregate index   | `https://{{TOOLS_HOST}}/llms.txt`            | `text/markdown`  | generated at deploy time from .html metadata; also the destination of `/`'s `Accept: text/markdown` redirect |

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
- Tools without URL-state deep-linking (`markdown-to-html`, `mock-saml-idp`)
  skip the wire format / encoder sections and use a "How to direct the user"
  section instead - see `markdown-to-html.skill.md` for the template.

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

| URL                                            | Content type        | Source of truth                            | Responder env var                                  |
|------------------------------------------------|---------------------|--------------------------------------------|-----------------------------------------------------|
| `/robots.txt`                                  | `text/plain`        | `buildRobotsTxt()` in `deploy.ts`          | `SECUTILS_HTML_APP_RESPONDER_ID_ROBOTS_TXT`         |
| `/sitemap.xml`                                 | `application/xml`   | `buildSitemapXml()` in `deploy.ts`         | `SECUTILS_HTML_APP_RESPONDER_ID_SITEMAP_XML`        |
| `/.well-known/agent-skills/index.json`         | `application/json`  | `buildAgentSkillsIndex()` in `deploy.ts`   | `SECUTILS_HTML_APP_RESPONDER_ID_AGENT_SKILLS_INDEX` |
| `Link:` headers on `/`                         | (HTTP response headers) | hard-coded `indexLinkHeaders` in `deploy.ts` | (no extra responder; pinned via index settings)   |

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
shape: `$schema` field plus a `skills` array where each entry has `name`,
`type: "skill"`, `description` (mirrors the HTML's `su-tool-description`),
`url` (the live `<path>.md` URL), and `sha256` of the deployed skill body.
The hash is computed from the **substituted** Markdown body that actually
ships, so an agent that's already cached the skill can detect updates with
a single GET.

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
.btn { padding: 7px 14px; border-radius: 8px; border: 1px solid var(--border); background: var(--surface); color: var(--text); font: 13px/1 var(--font); cursor: pointer; transition: all .15s; display: inline-flex; align-items: center; gap: 5px; }
.btn:hover:not(:disabled) { background: var(--surface-hover); border-color: var(--text-muted); }
.btn-primary { background: var(--primary); border-color: var(--primary); color: var(--primary-text); font-weight: 600; }
.btn-primary:hover:not(:disabled) { background: var(--primary-hover); border-color: var(--primary-hover); }
.btn-sm { padding: 5px 10px; font-size: 12px; }
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

- `.btn-sm`: `padding: 5px 10px; font-size: 12px;` (inherits `line-height: 1` from `.btn { font: 13px/1 ... }`) → 5+1+12+1+5 = 24
- `.view-tabs`: `padding: 2px;` + `border: 1px;` + `.view-tab { padding: 3px 10px; font: 12px/1; }` → 1+2+(3+12+3)+2+1 = 24
- `.icon-btn`: `padding: 4px;` + `svg 16x16` → 4+16+4 = 24

**The `/1` in `font: 12px/1 var(--font)` is load-bearing.** Without it the
`font` shorthand resets `line-height` to `normal` (~1.2-1.4 for Inter), so the
inner pill renders at ~21 px instead of 18 px and the outer view-tabs swells
to 27 px. The misalignment looks like the bar got taller, but the bar is
still 38 px - the pill's content box just outgrew the `.btn-sm` it's sitting
across the splitter from. Always pin `line-height` explicitly on any control
that lives in `.panel-bar`.

Do **not** change one in isolation - touching any of the three requires
checking the other two and the per-tool mobile override (`.btn-sm` shrinks
to 23 px on mobile in `markdown-to-html` via `padding: 5px 9px; font-size:
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
the `Export` icon in `markdown-to-html`. Either every Copy button has the
icon or none does - picking and choosing per tool produces the inconsistency
that previously made `saml-decoder` / `jwt-debugger` / `echo` look unrelated
to `markdown-to-html`.

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
`saml-decoder`, `markdown-to-html`) align the tops of both panel bodies by
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

Output panes that need to switch between rendered views (e.g. Markdown
preview vs. HTML iframe in `markdown-to-html`, XML vs. Attributes in
`saml-decoder`) use a segmented pill, not a border-bottom tab bar. The pill
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

Since branding is already in the header, the footer should contain a **short description of the tool** - not a "Powered by" watermark. Use `<p>` text, no logo repetition. Every footer also carries a **Privacy** link (a `<button>` that opens the canonical privacy dialog - see "Privacy dialog" below). The link is a `<button>` rather than an `<a href="#privacy">` so it doesn't pollute history or the URL fragment (the fragment is reserved for tool state, see "URL state encoding" above).

Two-line layout: the tool description on the first line, the Privacy link demoted to a smaller, dimmer second line so it reads as "fine print" rather than competing with the description.

```html
<footer class="su-footer">
    <p>A single-file tool description goes here.</p>
    <p class="su-footer-fineprint"><button type="button" class="su-footer-link" id="privacyOpen">Privacy</button></p>
</footer>
```

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
<script defer src="https://tools.secutils.dev/js/script.js"></script>
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
<dialog id="privacyDialog" class="su-dialog" aria-labelledby="privacyDialogTitle">
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

## Responsive (mobile)

```css
@media (max-width: 640px) {
    header { padding: 0 12px; }
    .logo-svg { height: 20px; }
    .logo-badge { font-size: 11px; padding: 2px 7px; }
    .btn { padding: 6px 10px; font-size: 12px; }
}
```

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

The reference implementation in `markdown-to-html.html` follows all of the above and is
the canonical example. When modifying an existing tool that still uses legacy syntax,
modernize the surrounding code in the same edit.

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
   `<main>` (see "More free tools bottom CTA"); footer with a **Privacy** button (see
   "Footer") backed by the canonical `<dialog>` block as the last child of `<body>`
   (see "Privacy dialog"). Do NOT add a separate `<nav class="su-related">` block -
   the banner is the sole related-tools surface.
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
The pattern, demonstrated in `markdown-to-html.html`, is:

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
