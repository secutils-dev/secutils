# JavaScript style and runtime patterns

Read before writing or reviewing tool JavaScript: syntax style, long tasks, busy-state gating for long pipelines, and visibility-aware polling.

## JavaScript Style

Inline scripts ship untranspiled to evergreen browsers: write modern, compact, idiomatic JavaScript.

**Required:**

- **`const` by default, `let` only when reassigned.**
- **Arrow functions** for callbacks and short helpers. Use named `function` declarations
  only for top-level helpers where the name aids readability or the function needs
  hoisting.
- **Template literals** for any string with interpolation, multi-line content, or HTML
  and CSS fragments.
- **`for…of`** over `Array.prototype.forEach` for plain iteration.
- **Spread syntax** to convert `NodeList` / iterables to arrays:
  `[...el.querySelectorAll('…')]`.
- **Optional chaining (`?.`) and nullish coalescing (`??`)** instead of `&&`/`||` chains
  when the intent is "value or fallback when null/undefined".
- **`async`/`await`** for clipboard, fetch, and any other promise-returning APIs. Avoid
  raw `.then()` chains unless the call site can't be `async`.
- **Destructuring** for object/array unpacking when it improves readability.
- **`catch {}`** (no binding) when the error is intentionally ignored.
- **Hoist constants** (CDN URLs, regexes, SVG markup, repeated HTML fragments) to
  module-top `const`s instead of inlining them at every use site.
- **Cache element references** in a single object rather than calling
  `document.getElementById` repeatedly; a tiny `const $ = (id) => document.getElementById(id);`
  helper plus a frozen `els = { … }` map keeps things tidy.

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

- [`dev/tools/md.html`](../md.html) `render()` —
  yields every 50 ms between hljs `highlightElement` calls. The 150 ms
  `updatePreview` debounce hides the async hop from the user.
- [`dev/tools/pdf-extractor.html`](../pdf-extractor.html) `renderShots()` and
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
[`dev/tools/pdf-extractor.html`](../pdf-extractor.html)):

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

Canonical shape (reference implementation: [`dev/tools/webhook.html`](../webhook.html)):

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
