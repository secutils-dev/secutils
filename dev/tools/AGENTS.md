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

Current mapping:

| File                       | `su-tool-path`            | Description         |
|----------------------------|---------------------------|---------------------|
| `index.html`               | `/`                       | Tool index page     |
| `jwt-debugger.html`        | `/jwt`                    | JWT Debugger        |
| `saml-decoder.html`        | `/saml`                   | SAML Decoder        |
| `mock-saml-idp.html`       | `/elastic/saml/idp-login` | Mock SAML IdP       |
| `certificate-decoder.html` | `/pem`                    | Certificate Decoder |
| `markdown-to-html.html`    | `/md-to-html`             | Markdown → HTML     |
| `echo.html`                | `/echo`                   | HTTP Echo Response  |

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

## Responder Script (`@su:responder-script`)

Most tools in `dev/tools/` are pure client-side HTML - the responder just serves a static
body. A few tools also need a small server-side script (e.g. `echo.html` decodes a `?c=…`
query parameter and returns a synthesised HTTP response). To keep the HTML the single
source of truth for both halves, embed the responder script in an HTML comment with the
`@su:responder-script` marker:

```html
<!DOCTYPE html>
<!-- @su:responder-script
// Optional human-readable preamble as JS // comments — these survive into the
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
  JS — and would also break HTML parsing — but worth knowing.
- **Single match per file**: only the first marker comment is used; additional ones
  produce a yellow `⚠ multiple @su:responder-script comments found, using the first`
  warning in the deploy log.
- **Marker is opt-in**: most tools are static HTML and don't need this - leave it off and
  deploy ships the body alone.

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

## Skill link (`skill.md`)

Each tool may publish a companion **AI agent skill** at `<su-tool-path>/skill.md`
(YAML frontmatter + markdown body, following the convention pioneered by
Anthropic Skills / Cursor skills / agents.md). The skill describes the tool's
inputs, wire format, and trigger phrases so an LLM can drive the tool
end-to-end without scraping the HTML UI.

The skill itself lives as a **separate sibling responder** (e.g. responder at
`/echo/skill.md` next to the responder at `/echo`). It is **not** part of the
`dev/tools/` deploy pipeline - author and update those skill responders by
hand. The HTML app's only job is to advertise the skill via a uniform header
button so humans can discover the file even though it's intended for AI use.

### Where the button goes

Header right area, **immediately before the theme toggle**. Same chrome as on
every other tool - the header is the one piece of layout that's identical
across the whole `dev/tools/` family, so a single placement covers everything.

The href is **derived at runtime** from `location.pathname`, so each tool's
markup is identical and works for any responder path:

```js
document.getElementById('skillLink').href = location.pathname.replace(/\/$/, '') + '/skill.md';
```

### Markup (copy verbatim)

Place inside `.header-right`, before the `<button class="theme-toggle">`:

```html
<a id="skillLink" class="skill-link" href="#" target="_blank" rel="noopener"
   title="View AI agent skill (skill.md, opens in new tab)"
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

### CSS (copy verbatim)

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

### When to opt out

Tools without a published `skill.md` (and no plan to publish one soon) should
**leave the markup off**. A visible button that 404s is a worse UX than no
button at all. Today this means [`dev/tools/index.html`](index.html) (a tool
list, not a tool) skips it. As you write new `skill.md` responders for `jwt`,
`pem`, etc., add the markup to the corresponding HTML at the same time.

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

### Logo SVG (copy this exactly)

```html
<svg class="logo-svg" height="24" role="img" viewBox="0 0 98 16" xmlns="http://www.w3.org/2000/svg">
    <path d="m3 0h10c1.662 0 3 1.338 3 3v10c0 1.662-1.338 3-3 3h-10c-1.662 0-3-1.338-3-3v-10c0-1.662 1.338-3 3-3z" fill="#fed047"/>
    <path aria-label="SU" d="m11.285 12q-1.12 0-1.728-0.608-0.608-0.61867-0.608-1.6747v-5.6107h1.152v5.6q0 0.59733 0.29867 0.93867 0.29867 0.34133 0.88534 0.34133 0.58667 0 0.88534-0.34133 0.29867-0.34134 0.29867-0.93867v-5.6h1.152v5.6107q0 1.0667-0.608 1.6747t-1.728 0.608zm-6.368 0q-1.152 0-1.8453-0.608-0.69334-0.608-0.69334-1.664h1.1307q0 0.58667 0.384 0.928 0.384 0.33067 1.024 0.33067 0.62934 0 0.992-0.34133 0.36267-0.34134 0.36267-0.90667 0-0.42667-0.23467-0.74667t-0.672-0.43733l-1.12-0.29867q-0.78934-0.21333-1.248-0.77867-0.448-0.56534-0.448-1.3547 0-0.64 0.27733-1.1093 0.288-0.48 0.81067-0.74667t1.216-0.26667 1.216 0.26667q0.53334 0.26667 0.82134 0.74667 0.29867 0.46933 0.29867 1.0987h-1.1307q0-0.50133-0.34133-0.8-0.33067-0.30933-0.864-0.30933t-0.864 0.30933q-0.32 0.29867-0.32 0.78934 0 0.39467 0.21333 0.66134 0.224 0.26667 0.61867 0.37333l1.152 0.30933q0.81067 0.21333 1.28 0.832t0.46933 1.4613q0 0.69334-0.30933 1.2053-0.30933 0.50133-0.864 0.77867-0.55467 0.27733-1.312 0.27733z" fill="#642340"/>
    <path class="logo-text-fill" aria-label="SECUTILS.DEV" d="m93.158 12.117-1.9733-7.7867h1.1733l1.2587 5.184q0.11733 0.46933 0.20267 0.91734 0.08533 0.448 0.128 0.69334 0.04267-0.24533 0.128-0.69334 0.096-0.45867 0.21333-0.928l1.2053-5.1733h1.184l-1.984 7.7867zm-7.8294 0v-7.7867h4.576v1.024h-3.4453v2.2187h3.072v0.992h-3.072v2.528h3.4453v1.024zm-6.5174 0v-7.7867h2.176q0.768 0 1.3333 0.29867 0.56534 0.288 0.87467 0.832 0.32 0.53334 0.32 1.248v3.008q0 0.736-0.32 1.2693-0.30933 0.53334-0.87467 0.832-0.56534 0.29867-1.3333 0.29867zm1.152-1.0347h1.024q0.62934 0 1.0027-0.36267 0.37333-0.36267 0.37333-1.0027v-3.008q0-0.61867-0.37333-0.98134-0.37334-0.37333-1.0027-0.37333h-1.024zm-5.2374 1.1413q-0.416 0-0.68267-0.24533-0.256-0.256-0.256-0.672 0-0.416 0.256-0.672 0.26667-0.26667 0.68267-0.26667 0.416 0 0.672 0.26667 0.26667 0.256 0.26667 0.672 0 0.416-0.26667 0.672-0.256 0.24533-0.672 0.24533zm-6.368 0q-1.152 0-1.8453-0.608-0.69334-0.608-0.69334-1.664h1.1307q0 0.58667 0.384 0.928 0.384 0.33067 1.024 0.33067 0.62934 0 0.992-0.34133 0.36267-0.34134 0.36267-0.90667 0-0.42667-0.23467-0.74667t-0.672-0.43733l-1.12-0.29867q-0.78934-0.21333-1.248-0.77867-0.448-0.56534-0.448-1.3547 0-0.64 0.27733-1.1093 0.288-0.48 0.81067-0.74667 0.52267-0.26667 1.216-0.26667 0.69334 0 1.216 0.26667 0.53334 0.26667 0.82134 0.74667 0.29867 0.46933 0.29867 1.0987h-1.1307q0-0.50134-0.34133-0.8-0.33067-0.30933-0.864-0.30933t-0.864 0.30933q-0.32 0.29867-0.32 0.78934 0 0.39467 0.21333 0.66134 0.224 0.26667 0.61867 0.37333l1.152 0.30933q0.81067 0.21333 1.28 0.832 0.46934 0.61867 0.46934 1.4613 0 0.69334-0.30934 1.2053-0.30933 0.50134-0.864 0.77867-0.55467 0.27733-1.312 0.27733zm-8.288-0.10667v-7.7867h1.152v6.7414h3.4027v1.0453zm-6.6987 0v-1.0453h1.568v-5.696h-1.568v-1.0453h4.32v1.0453h-1.5787v5.696h1.5787v1.0453zm-4.8214 0v-6.7414h-2.08v-1.0453h5.3227v1.0453h-2.0907v6.7414zm-5.824 0.10667q-1.12 0-1.728-0.608-0.608-0.61867-0.608-1.6747v-5.6107h1.152v5.6q0 0.59733 0.29867 0.93867 0.29867 0.34133 0.88534 0.34133 0.58667 0 0.88534-0.34133 0.29867-0.34134 0.29867-0.93867v-5.6h1.152v5.6107q0 1.0667-0.608 1.6747t-1.728 0.608zm-6.3147 0q-1.0987 0-1.7493-0.608-0.64-0.61867-0.64-1.664v-3.456q0-1.056 0.64-1.664 0.65067-0.608 1.7493-0.608 1.088 0 1.728 0.61867 0.65067 0.608 0.65067 1.6533h-1.152q0-0.608-0.33067-0.928-0.32-0.32-0.896-0.32-0.58667 0-0.91734 0.32-0.32 0.32-0.32 0.928v3.456q0 0.608 0.32 0.928 0.33067 0.32 0.91734 0.32 0.576 0 0.896-0.32 0.33067-0.32 0.33067-0.928h1.152q0 1.0453-0.65067 1.664-0.64 0.608-1.728 0.608zm-8.6827-0.10667v-7.7867h4.576v1.024h-3.4453v2.2187h3.072v0.992h-3.072v2.528h3.4453v1.024zm-4.1707 0.10667q-1.152 0-1.8453-0.608-0.69334-0.608-0.69334-1.664h1.1307q0 0.58667 0.384 0.928 0.384 0.33067 1.024 0.33067 0.62934 0 0.992-0.34133 0.36267-0.34134 0.36267-0.90667 0-0.42667-0.23467-0.74667-0.23467-0.32-0.672-0.43733l-1.12-0.29867q-0.78934-0.21333-1.248-0.77867-0.448-0.56534-0.448-1.3547 0-0.64 0.27733-1.1093 0.288-0.48 0.81067-0.74667 0.52267-0.26667 1.216-0.26667 0.69334 0 1.216 0.26667 0.53334 0.26667 0.82134 0.74667 0.29867 0.46933 0.29867 1.0987h-1.1307q0-0.50134-0.34134-0.8-0.33067-0.30933-0.864-0.30933t-0.864 0.30933q-0.32 0.29867-0.32 0.78934 0 0.39467 0.21333 0.66134 0.224 0.26667 0.61867 0.37333l1.152 0.30933q0.81067 0.21333 1.28 0.832 0.46934 0.61867 0.46934 1.4613 0 0.69334-0.30934 1.2053-0.30933 0.50134-0.864 0.77867-0.55467 0.27733-1.312 0.27733z"/>
</svg>
```

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
```

## Footer

There are two different footer patterns depending on whether the page has the Secutils header or not:

### Tool app pages (have the Secutils logo header)

Since branding is already in the header, the footer should contain a **short description of the tool** - not a "Powered by" watermark. Use `<p>` text, no logo repetition.

```html
<footer class="su-footer">
    <p>A single-file tool description goes here.</p>
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
```

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

These tools target evergreen browsers (current Chrome / Firefox / Safari / Edge) — no
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
- **`catch {}`** (no unused binding) when the error is intentionally ignored — never
  `catch (e) {}` with an unused `e`.
- **Hoist constants** (CDN URLs, regexes, SVG markup, repeated HTML fragments) to
  module-top `const`s instead of inlining them at every use site.
- **Cache element references** in a single object rather than calling
  `document.getElementById` repeatedly; a tiny `const $ = (id) => document.getElementById(id);`
  helper plus a frozen `els = { … }` map keeps things tidy.

**Avoid:**

- `var` — `const`/`let` are the only acceptable bindings.
- `function () {}` callbacks — use arrow functions.
- String concatenation with `+` for HTML / CSS / multi-line text.
- Manual `Array.from(nodeList)` — use `[...nodeList]`.
- Truthy/falsy `&&`/`||` for null-fallbacks where `??` is the correct operator.
- `e` in `catch (e) {}` when unused — drop the binding.

**Optional but encouraged:**

- Top-level `await` is fine inside an `async` IIFE if the script needs it.
- Promise-wrap legacy event-driven APIs (e.g. `iframe.onload`, paged.js polling) so the
  control flow reads top-to-bottom.
- Use private object short-hand (`{ foo, bar }`) and computed property names where they
  make code clearer.

The reference implementation in `markdown-to-html.html` follows all of the above and is
the canonical example. When modifying an existing tool that still uses legacy syntax,
modernize the surrounding code in the same edit.

## Reference Implementation

See `dev/tools/markdown-to-html.html` for the complete working example.

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
  const re = /<script(?:[^>]*)>([\s\S]*?)<\/script>/g;
  let m, idx = 0, allOk = true;
  while ((m = re.exec(html))) {
    const code = m[1];
    if (!code.trim()) { idx++; continue; }
    if (/src=/.test(html.slice(m.index, m.index + 200))) { idx++; continue; }
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

## PDF Export (optional)

Tools that produce printable artifacts (rendered articles, decoded certificates, JWT
breakdowns, etc.) can offer a PDF export without breaking the single-page-HTML constraint.
The pattern, demonstrated in `markdown-to-html.html`, is:

1. Add a `↓ PDF` action button next to the existing download / copy actions.
2. Build a self-contained printable HTML document (same brand fonts and palette as the
   on-screen preview) with the article content and a small `<script>` tag that lazy-loads
   [Paged.js](https://pagedjs.org) from a CDN. Paged.js paginates the document into A4
   pages using CSS Paged Media (`@page`, `@bottom-center`, `string-set`, etc.) — vector
   text, selectable, fully styled to match the preview.
3. Inject that document into a hidden, off-screen `<iframe>` via `srcdoc`.
4. Inside the iframe, listen for `pagedjs:rendered` on `window` and flip a `__suPdfReady`
   flag; the parent polls that flag, then calls `iframe.contentWindow.print()`. The
   browser's native "Save as PDF" produces a vector PDF that is byte-for-byte consistent
   with the preview.
5. Clean up the iframe on `afterprint` (with a 30 s safety timeout for browsers that don't
   fire it reliably).

No WASM, no server round-trip, ~150 KB CDN script loaded only on first export. Always force
`data-theme="light"` on the print document — yellow-on-dark is great on screen but reads
poorly on paper.
