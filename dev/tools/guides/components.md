# Dialogs, popovers, and form validation

Copy-verbatim patterns for the Privacy and Credits dialogs, backdrop dismissal, tethered popovers, and form validation feedback. Read before adding or changing any of them.

For MARS and ECHO, use [design-system.md](design-system.md) for CSS and glyphs.
Keep the native behavior and accessibility contracts from this guide.

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

The matching footer button is documented in [chrome.md](chrome.md#footer)
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
that. The two tools still on ES5-era syntax (`var` / `function ()`) -
`index.html` and `certificate-decoder.html` - mirror the same style with `var`
instead of `const`; the behaviour is identical. New tools use the modern form
(see [javascript.md](javascript.md)).

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

If a tool ships **zero** third-party browser JS (today: `index.html`,
`echo.html`, and `webhook.html`), drop both the footer button and the
`<dialog>` block - there is nothing to credit. Vendored code that runs only inside the
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
form control whose own click handler stopped propagation". The ES5-style tools
(`index.html`, `certificate-decoder.html`) mirror the same logic with `var` and
IIFE hoisting; the behaviour is identical and the LOC budget still holds.

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
three engines in late 2025. Both qualify under the browser policy in [`../AGENTS.md`](../AGENTS.md#contracts).

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
- Leave `aria-expanded` off the invoker: the user agent reflects expanded state into the accessibility tree when the
  invoker is associated with a popover via `commandfor`. The literal HTML attribute
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

Leave `z-index` unset on the popover: shown `[popover]` elements are promoted to the top layer; they sit above any positioned siblings without
any z-index dance and ignore `overflow: hidden` on their containing scroll
parent. This is the main reason to migrate.

**Why not CSS Anchor Positioning (`position-area`)?** It would replace the
JS helper below with a one-liner, but as of Q2 2026 anchor positioning is
still Chromium-only (Firefox / Safari have partial or flagged support). It
is **not** Newly Baseline, so it falls outside the browser policy. Once it
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

- [`dev/tools/md.html`](../md.html) — HTML options gear
  popover + Export dropdown menu (the canonical pair).
- [`dev/tools/pdf-extractor.html`](../pdf-extractor.html) — OCR settings popover
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

- [`dev/tools/echo.html`](../echo.html) — status-code number input + header value
  inputs; URL-hydrated state.
- [`dev/tools/mock-saml-idp.html`](../mock-saml-idp.html) — required username +
  email form.
