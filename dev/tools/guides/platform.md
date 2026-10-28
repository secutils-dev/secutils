# Tool metadata, hosts, responder scripts, and bundles

How a tool's HTML declares where it is served and how `deploy.ts` turns it into responders. Read before adding or renaming a tool, changing `su-tool-*` metadata or hosts, writing an `@su:responder-script`, or adding a `dev/tools/js/` bundle.

## Metadata (`su-tool-*`)

Every tool HTML, `index.html` included, declares itself in `<head>`:

```html
<meta name="su-tool-path" content="/jwt">                  <!-- URL path of the responder; the filename may differ -->
<meta name="su-tool-name" content="JWT Debugger">
<meta name="su-tool-description" content="One-line description, reused by llms.txt and the agent-skills index.">
<meta name="su-tool-promote" content="true">
```

Write the description in plain text: `deploy.ts` copies the attribute value verbatim, so an entity such as `&amp;` reaches
`llms.txt` and the agent-skills index as literal text.

`e2e/tools/registry.ts` repeats name, path, description, and promotion, and adds the OG and E2E data the page doesn't carry
(accent color, OG icon, `applicationCategory`, `host`). The OG generator and every `e2e/tools/` spec read it.
When a tool is added, removed, or renamed, update in one change: its `<meta>` tags, its registry row, and its
`index.html` card (when promoted):

```html
<a class="tool-card" href="/the-path">
    <div class="tool-name">Tool Name <span class="tool-path">/the-path</span> <span class="arrow">&rarr;</span></div>
    <div class="tool-desc">Short description of what the tool does.</div>
</a>
```

## Promotion (`su-tool-promote`)

- `true`: listed on `index.html` (card and JSON-LD `ItemList`), in the README "Free single-page tools" table, on the
  marketing site's `#free-tools` cards, and in the aggregates `deploy.ts` generates (`llms.txt`, `sitemap.xml`,
  `.well-known/agent-skills/index.json`). Use it for every user-facing tool.
- `false`: listed nowhere, and the page carries `<meta name="robots" content="noindex, nofollow">`. The page, its skill,
  and Markdown negotiation still deploy, so a shared direct link keeps working. Use it for niche tools such as
  `mock-saml-idp` (Elasticsearch and Kibana SSO testing).

`make tools-check` asserts the marketing and README side: every promoted tool is linked from both and has a sibling skill,
and no unpromoted tool leaks onto the marketing page. Point `SECUTILS_TOOLS_PROMO_HOME_INDEX` at the marketing site's
`index.html` (a sibling checkout); without it, that part of the check is skipped with a warning.
`e2e/tools/index.spec.ts` and `registry.spec.ts` assert the deployed side: the index page and every aggregate list exactly
the promoted tools.

## Hosts (`{{TOOLS_HOST}}`, `{{MARS_HOST}}`)

`deploy.ts` substitutes `{{TOOLS_HOST}}` (`SECUTILS_TOOLS_PUBLIC_HOST`, default `tools.secutils.dev`) and `{{MARS_HOST}}`
(`SECUTILS_MARS_PUBLIC_HOST`, default `mars.secutils.dev`) in every `.html` and `.skill.md` before minification:
titles, canonical URLs, `og:url`, JSON-LD, links, and skill URLs all use the placeholders.
The marketing site templates `{{ TOOLS_HOST }}` through its own Parcel build; it has no MARS placeholder, so it links
MARS by its absolute URL.

An **own-host app** is served at `/` of its own host instead of a path on the tools host.
It declares `<meta name="su-tool-host" content="{{MARS_HOST}}">` with `su-tool-path` `/`, and its SKILL.md is served
unchanged at `/llms.txt` of that host. `deploy.ts` then negotiates `Accept: text/markdown` to `/llms.txt`, lists the app
in the tools-host `llms.txt` and agent-skills index by its own URLs, and leaves it out of the tools-host `sitemap.xml`
(a sitemap lists one host). `tools-check.ts` and the e2e registry (`host`, `toolUrl`, `skillUrl`) follow the same rule.
MARS is the only own-host app.

## Responders

`deploy.ts` PUTs only a responder's `settings` (body, script, headers, `requestsToTrack: 0`), never its `location`.
Set the location when creating the responder (`POST /api/webhooks/responders` or the Web UI):

- `subdomainPrefix: "tools"` and `path: "/<path>"` for a tools-host page; `path: "/<path>.md"` with `pathType: "="` for its skill.
- `pathType: "^"` (prefix) for a page that routes on sub-paths (`webhook.html` serves `/webhook/<token>`), or for an
  own-host app at `/`. An exact-match page answers its bare path, and every sub-path gets an empty `404`.
  Switch an existing responder with a `PUT` carrying only `location`:

  ```bash
  curl -X PUT "$API/api/webhooks/responders/$RID" -H "Authorization: Bearer $KEY" \
    -H 'Content-Type: application/json' \
    -d '{"location":{"pathType":"^","path":"/webhook","subdomainPrefix":"tools"}}'
  ```

The responder IDs live in `.env`: `SECUTILS_HTML_APP_RESPONDER_ID_<NAME>` for `<name>.html`, `..._<NAME>_MD` for
`<name>.skill.md`, and one ID per aggregate (`_INDEX`, `_LLMS_TXT`, `_ROBOTS_TXT`, `_SITEMAP_XML`, `_AGENT_SKILLS_INDEX`).
A file without an ID is skipped with a warning.

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

`deploy.ts` takes the first comment whose content starts with `@su:responder-script` and PUTs its body as
`settings.script`, next to the minified HTML as `settings.body`; the minifier strips the comment from the page.
`ResponderSettings` is replaced wholesale, so a file without the marker clears any script on its responder.
The deploy log reports the script size beside the body size, and warns when a file has more than one marker.

Rules:

- Write the script as JavaScript from the marker line to the first `-->`: preamble lines are `//` comments, and the body
  never contains `-->`.
- Every HTML responder script is wrapped by the Markdown-negotiation prelude ([agent-surface.md](agent-surface.md#markdown-content-negotiation-accept-textmarkdown)),
  which may answer first with a `302`; the tool's script runs only when it doesn't.
- Return `skipRequest: true` on every non-null response, through one `json()` helper so none is missed, so request
  history never records user data. `deploy.ts` also sends `requestsToTrack: 0`. A script that throws is still tracked,
  so turn error paths into explicit responses.
- `context.body` is a `Uint8Array` outside the V8 heap: pass it to `Deno.core.decode` directly
  (`Array.from(context.body)` costs about 8 B per body byte of the 10 MiB heap).
- A page that routes on sub-paths needs a prefix responder ([Responders](#responders)).

## Embedded JS bundles (`data-su-bundle`)

Shared visual assets use a separate inclusion step, documented in [design-system.md](design-system.md).
Deployment inlines them before minification, without a new app bundle.

When a tool needs an npm package (Node-only dependencies to stub, a worker, WASM, or a multi-file app), build it in a
sub-package and let `deploy.ts` inline the output into the page. Current bundles: `liteparse` (`pdf-extractor.html`),
`micro-ml` (`forecast.html`), `three-mf` (`md.html`, `mars.html`), and `mars` (the MARS app).

Sub-package layout, isolated from the root `package.json`:

```
dev/tools/js/<name>/
  package.json          # own deps, pinned; scripts.build
  package-lock.json
  src/                  # source and stubs
  dist/<name>.js        # build output (gitignored, like node_modules/)
  README.md             # what it bundles and which upstream versions are pinned
```

The contract is `npm run build` producing one self-contained ESM file at `dist/<name>.js`; the bundler and stubs are the
sub-package's choice.

Placeholder in the page, empty, with `gzip-base64` as the default encoding:

```html
<script id="su-bundle-foo" type="text/plain" data-su-bundle="foo" data-su-bundle-encoding="gzip-base64"></script>
```

- `type="text/plain"` keeps the source from running at page load; the tool imports it on first use, so visitors who
  never reach that feature pay only the download.
- `data-su-bundle` names the sub-directory (`[a-z0-9_-]+`).
- `gzip-base64` keeps large bundles under the responder body and PUT-payload caps (about 4-5x smaller, roughly 10-20 ms to
  decompress). Raw inlining (no encoding attribute) suits only small bundles.

Loader (copy verbatim):

```js
async function loadSuBundle(name) {
    const el = document.getElementById(`su-bundle-${name}`);
    if (!el?.textContent) throw new Error(`Bundle "${name}" is not loaded`);
    let src = el.textContent.trim();
    if (el.dataset.suBundleEncoding === 'gzip-base64') {
        const bytes = Uint8Array.from(atob(src), (c) => c.charCodeAt(0));
        src = await new Response(new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'))).text();
    }
    return import(URL.createObjectURL(new Blob([src], { type: 'text/javascript' })));
}
// const { LiteParse } = await loadSuBundle('liteparse');
```

At deploy, after minification, `deploy.ts` finds each placeholder, rebuilds `dist/<name>.js` when any source under the
sub-package is newer (running `npm ci` first when `node_modules/` is absent), and inlines it (escaping `</script>` in raw
mode). It refuses a placeholder that already has content. `make tools-bundles` pre-builds every sub-package.
