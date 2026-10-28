# Visual chrome

Read before changes to tool layout, analytics, resource priority, or exported documents.
MARS, ECHO, and Webhook Inspector use [design-system.md](design-system.md) for visual primitives.
That guide takes precedence over the color, button, and icon snippets here for migrated tools.
The snippets below describe legacy tool chrome, with `md.html` as the reference.

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
- Keep `balance` on the listed selectors only: the balancing algorithm runs a binary search on line widths, so it is
  priced per element, and the list targets a few dozen at most.
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

For SVG artwork that touches its viewport edges, read [SVG viewport clipping](design-system.md#svg-viewport-clipping).

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
[`dev/tools/index.html`](../index.html)'s `<a class="logo">` block. Only the wrapping
`<a>` and the height attribute matter for new tools (height `24` everywhere except
`20` on mobile, controlled by the `.logo-svg` rule in the responsive section).

## Dark/Light Theme Toggle

- Use EUI SVG icons (from `node_modules/@elastic/eui/lib/components/icon/svgs/`)
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

### Control heights (load-bearing)

Controls in the same row must compute to the same outer height, or mixed rows (a `Share` `.btn-sm` across the splitter
from an `XML | Attributes` `.view-tabs` pill, `Options` next to `Parse`) visibly drift by 1-6 px. Change one of these
together with the others and the mobile overrides:

- `.btn-sm` is pinned at `height: 24px` and `.btn` at `height: 29px`. The pin is what keeps `.btn-primary` level with the
  regular variant: Inter's heavier weight enlarges the line box, and `line-height: 1` alone doesn't clamp it.
- `.view-tabs` (`padding: 2px; border: 1px`) with `.view-tab { padding: 3px 10px; font: 12px/1 }` and `.icon-btn`
  (`padding: 4px` around a 16 px icon) both come to 24 px. Keep the `/1` in every `font` shorthand of a `.panel-bar`
  control: without it, `line-height` resets to `normal` and the pill grows to 27 px.
- `.btn-primary` uses `font-weight: 500` and `border-color: var(--primary-text)` (the maroon, contrast-safe in both
  themes), so it reads the same size as the outlined `.btn` beside it. `mock-saml-idp.html` keeps a +100 weight delta
  over its 500-weight regular buttons.
- A mobile override that shrinks `.btn-sm` (as `md.html` and `pdf-extractor.html` do with `padding: 5px 9px; font-size: 11px`)
  also pins `height: 23px`, matching the 22-23 px mobile `.view-tab`.

### Panel bars and breakpoints (load-bearing)

- `.panel-bar` pins `flex-shrink: 0`. Without it, a sibling with a large flex basis (a `height: 100%` textarea) squeezes
  one pane's bar by 2-3 px and the two panes' bodies fall out of alignment. The general rule: a fixed-height flex item
  that matters for cross-pane alignment pins `flex-shrink: 0`.
- Two-pane tools stack their columns at `max-width: 900px` (below it each column is too narrow for the bar). There the bar
  wraps, and its fixed height becomes `height: auto; min-height: 38px` so a wrapped second row stays inside the bar.
- Phone-size chrome tweaks live in a separate `max-width: 640px` query:

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
icon or none does.

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
`saml-decoder`, `jwt-debugger`, `pdf-extractor`, `forecast` - grep
`.panel-bar` for the current set) align the tops of both panel bodies by
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
  (`.view-tabs`) when the pane has multiple views; with tabs, the tab names carry the label. Keep labels to a single short
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
section, so it inherits `<main>`'s content padding instead of double-padding against the footer.

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

Since branding is already in the header, the footer should contain a **short description of the tool** - not a "Powered by" watermark. Use `<p>` text, no logo repetition. Every footer also carries a **Privacy** link and (when the tool ships any third-party browser-runtime JS) a sibling **Credits** link, both `<button>` elements that open the canonical dialogs - see [components.md](components.md#privacy-dialog-footer) and [the Credits dialog](components.md#credits-dialog-footer). The links are `<button>` rather than `<a href="#privacy">` so they don't pollute history or the URL fragment (the fragment is reserved for tool state, see [url-state.md](url-state.md) above).

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

Tools whose browser side ships **zero** third-party JS (today: `index.html`, `echo.html` - tiny-inflate is server-side only - and `webhook.html`) omit the Credits link and the middle-dot separator, leaving the fineprint as a single Privacy button.

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

### Generated/exported output files (no Secutils header, e.g. `md.html`'s HTML export)

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
  serves the tools. **This is the one deliberate exception to the
  `{{TOOLS_HOST}}` rule** - every tool hard-codes the literal host here rather
  than templating it, because the reverse-proxy route to Plausible is pinned to
  `tools.secutils.dev` in infra, not derived from the deploy variable. If
  `SECUTILS_TOOLS_PUBLIC_HOST` is ever changed, this URL must be updated in every
  `dev/tools/*.html` **and** the proxy route moved with it; templating it would
  silently point analytics at a host with no `/js/script.js` behind it.
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
  dashboard attributes events to the page's host (`tools.secutils.dev`; MARS reports as `mars.secutils.dev`, a
  separate Plausible site). The
  inline shim queues any `plausible(...)` calls made before the loader
  arrives, so future custom events (e.g. `plausible('Copy share link')`)
  buffer cleanly.

### How the deploy pipeline treats the snippet

`html-minifier-terser` strips the `<!-- Privacy-friendly analytics by Plausible -->`
comment via `removeComments: true` and re-emits both `<script>` tags as-is (the
`src="..."` script tag is preserved, the inline init shim goes through
`minifyJS`). No special handling in [`deploy.ts`](../deploy.ts) is required.

### Why inline-HTML rather than the dynamic-injection pattern used on `secutils.dev`

The main `secutils.dev` marketing site injects the same Plausible script
dynamically from a TypeScript entry. That works because its Parcel build
bundles the entry into a single JS file. The tools are single static HTML
files with no per-page build step beyond `html-minifier-terser`, so inlining
the snippet is simpler, deterministic, survives minification, and gives
every page the analytics loader before the inline init shim runs (rather
than after a paint).

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
"Typography polish" rules above, and ships in every tool
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
