# MARS (`mars.html`)

Read before changing MARS: `mars.html`, `mars.skill.md`, `js/mars/`, or `mars/assets/` (paths relative to `dev/tools/`).

MARS (Markdown Annotation & Review System) is the review app extracted from `md.html`. It is the one own-host app
([platform.md](platform.md#hosts-tools_host-mars_host)): served at `/` of `{{MARS_HOST}}` (`mars.secutils.dev`, a
prefix-match `_MARS` responder) with its SKILL.md at `/llms.txt` (an exact-match `_MARS_MD` responder).
It is promoted like any tool, linked by that host. In-app URLs derive from `location.pathname`, so the page works on any host.

## How it differs from the other tools

- **Source layout**: `mars.html` is a thin shell (SEO head, static DOM, the `@su:responder-script`, favicons) plus two
  `gzip-base64` bundles: `mars` (the app, a TypeScript sub-package in `js/mars/`, built by esbuild) and `three-mf`
  (shared with `md.html`). The shell imports the `mars` bundle at startup; `html.mars-boot` hides the body until the
  bundle has injected its CSS.
- **Look**: EUI-styled (Borealis tokens, EUI icons, a header with a docked sidebar like the Web UI), instead of the
  legacy tools chrome: Privacy and Credits sit in the sidebar, and there is no CTA banner or footer.
  Reusable primitives and glyphs live in `shared/` ([design-system.md](design-system.md)). The brand ramp
  (`--mars-*`, `--brand*`) follows the logo and lives in `js/mars/src/ui/tokens.css`. The logo artwork lives in
  `mars/assets/`; the header mark and data-URI favicons in `mars.html` are copies of it.
- **Sidebar links**: documents and search results contain real anchors to `#doc=<id>`. Ordinary right-click opens MARS
  actions. Shift + right-click exposes the browser link menu. Modified clicks retain browser navigation.
  The action button and rename input stay outside the anchor. The link is the document's keyboard focus target.
  Pointer navigation flushes pending edits and defers tree replacement until the gesture finishes.
  Links require the same browser profile. Private-window storage and native split-view availability belong to the browser.
- **Viewer**: `js/mars/src/doc/viewer.js` is a copy of `md.html`'s live script, without its output UI or fragment autosave.
  `viewer.d.ts` is its host API. Maps and models now use `src/doc/diagrams.js`, shared with standalone export runtimes.
  A fix to rendering, block editing, formatting, or comments in
  either file lands in both: after porting, diff the shared sections of the two scripts. The "Review changes hooks"
  section of `viewer.js` (`onRendered`, `setThreadHooks`, `blockText`, `requote`, and the few call sites in
  `updatePreview`, `anchorThreads`, and the thread cards) is MARS-only and stays out of `md.html`.
- **Tables**: `src/doc/table*.ts` owns source operations, proportional layout, and Preview controls.
  Table resizing and structural editing are MARS-only by user decision, so these hooks stay out of `md.html`.
  The shared block mapper retains byte-parity apart from the explicit table hook calls.
  Advance table ranges past source-mapped code/HTML tokens, including inline list fences, before searching for a table.
  Store proportions in an adjacent `su-table` comment, not IndexedDB side data or a new link format.
  Keep metadata with whole-table block moves/deletion, and preserve cell Markdown and alignment during column moves.
  Cell edits splice only the selected content. Alignment edits change only divider colons.
  Preview selection formatting uses the selected cell's source range, not the complete table or a header URL.
  Apply cell drafts before view changes, sharing, or export. Discard stale drafts after outside source changes.
  Preview and export rendering use the same layout helper. Add controls only after the live Preview renders.
  Cancel pointer gestures on source changes, navigation, Source mode, and export capture.
  Read [the package table section](../js/mars/README.md#table-editing) for the format and lifecycle.
- **Export**: `src/export/` owns the HTML/PDF workspace. It captures one immutable document and shares export-safe rendering
  across formats. HTML previews and downloads use the same output. PDF preview and printing use the same prepared pages.
  Settings drafts remain separate from document state and commit to `mars.exportSettings` after an output action.
  Reader and print runtimes compile separately to inline source strings. Paged.js loads only for PDF pagination.
  Preview frames use opaque origins, with source-window and operation checks on messages.
  Keep print-only CSS compatible with the Paged.js parser: expand heading selectors instead of using comma lists in `:is()`.
  Add screen preview scaling after pagination, so Paged.js does not rewrite its media rules.
  Read [the package export section](../js/mars/README.md#export-workspace) for lifecycle and network contracts.
- **Source input**: `src/io/source.ts` extracts embedded Markdown without changing viewer state.
  Open, drops, imports, Load URL, and `?url=` share source extraction and naming.
  Explicit incoming names precede frontmatter titles, Markdown headings, and filename fallbacks.
  Converted HTML never retains a writable file handle. HTML without embedded source stays raw HTML.
  `src/io/input.ts` owns input generations and abort signals. Pass the current operation through receiving and navigation.
  Check it after asynchronous reads and before selecting or changing a document.
  Both URL entry points use `share.receiveUrl()`, with browser-side fetches and the existing incoming-copy decisions.
  Unchanged Markdown and review blocks retain their original bytes on download.
  PDF Extractor sends Text and Markdown through a full link with an explicit source name.
  Keep the handoff browser-side, with exact text in the fragment and no PDF bytes.
- **Incoming copies** (`share.updateExisting`): a link, a `mars` re-open, or a file from Open or a drop that matches a
  stored document by origin, by file handle (`isSameEntry`), or, after the reader confirms, by content
  (`share/match.ts`). The text is decided against `opened`: only the source changed takes it, only the stored copy
  changed keeps it, and both changed asks (default **Save as a copy**). **Make independent** removes the source and gives the
  document a fresh origin.
- **Review changes**: an outside change to a document's text keeps the previous text as a baseline (`app.rebase`).
  `doc/changes.ts` diffs it against the preview (`doc/diff.ts`) until **Mark reviewed**; **Show changes since opened**
  diffs against `opened` instead.
- **Linked files** need the File System Access pickers, so they exist in Chromium only. Mozilla's position on them is
  negative and WebKit opposes them, so plan no Firefox port of the poll. The 2-second poll of the open linked file only
  offers a reload unless the reader turns on **Reload linked files automatically** (`mars.liveFiles`). The one route to
  live updates in Firefox is outside the browser APIs: a `mars --watch` helper serving the file on `localhost`.
- **Search** (`search/`): the sidebar filter matches names and text. Smart search is opt-in: Transformers.js runs an
  embedding model in a module worker, both lazy-loaded from jsDelivr and Hugging Face. The chosen model sits in
  `mars.smartSearch`, and the suggestion to turn it on sits in `mars.smartNudge`. The page is cross-origin isolated
  (COOP `same-origin`, COEP `credentialless`) so models get CPU threads; a new cross-origin embed must load under
  `credentialless` (no credentials) or it breaks.
- **Storage**: documents live in IndexedDB (database `mars`: `nodes`, `docs`, `meta`). Per-document side data sits
  in `docs` under `<kind>:<id>` (`db.getSide`), so it needs no new store: `base` (review baseline, flagged by `pending`
  on the node), `opened` (the source text as last received, flagged by `opened`), and `vec` (smart-search index). The fragment is a route
  (`#doc=<id>`) or an incoming share in persistent sessions. Every document carries an `originId` that survives sharing;
  opening a link for a known `originId` merges its review threads (by comment `id`, or `author` plus `at`).
  Open existing databases without a requested version. Some browsers retain version 2 from the deferred review-rounds feature.
  Preserve extra stores during rollback. Never downgrade or delete the database to restore the previous UI.
- **Connections**: `src/sync/` owns responder-only sync and live links. `sync:<id>` side records store replicas, outboxes, keys, and received progress.
  `db.commitReplica` atomically saves their Markdown projection and progress. Existing stores and database versions remain unchanged.
  Collection/member-scoped metadata keeps live grants separate from an owner's private connection in the same profile.
  Read [the sync contracts](../js/mars/src/sync/README.md) before changing grants, key epochs, or the outbox.
  MARS-specific sync hooks stay out of Markdown Preview. Source-mapping parity tests remove only explicit hook lines.
  Local preview and deployment use `mars-sync-assets.ts` to assemble the same sandbox script.
  A live fragment selects memory storage before IndexedDB bootstrap. Explicit library saving uses an atomic promotion.
  New device-pairing links use short `#sync=` fragments with encrypted bootstraps. Long `#sync=` and `#invite=` links require independent wrapping keys.
  Sync manages personal devices. Live links manages source-side sharing, and Live document manages a recipient's retained copy.
  The common Copy link dialog handles document/folder snapshot formats and document-only Live sharing. Keep link-security explanations in Privacy.
  It defaults to Snapshot and Full. Expiring links offer 1/7/14/30 days without calendar dates.
  Source Live-link management sits in this dialog's footer. The generated URL replaces a clear action instruction without changing the result area's height.
  The Share menu's Copy link item keeps a short label and exposes its helper text as an accessible description.
  Recipients forward an active owner's URL through Copy link → Live, with fixed access and expiry. Ended links offer snapshots only.
  Folder activity renewal includes nested folder metadata as well as document replicas.
  Download and Export all offer optional encrypted connection recovery. Normal exports remain credential-free.
  Live-only teardown purges encrypted data and releases capacity after the last link is disabled. Personal Sync remains separate.
  Protocol-only commits retain document timestamps and existing smart-search indexes.
  New Live links use encrypted KV bootstraps and a short fragment secret. Full links require independent wrapping keys.
  Library promotion transfers the selected vector record and reloads the persistent index without invalidating unchanged documents.
- **Temporary sessions**: `t=1` in the fragment or `?temporary=1` selects the in-memory backend before database bootstrap.
  Denied storage and write-quota failures also select it. Keep the persistent database unchanged, including its last-document metadata.
  `share/temporary.ts` stores only the current document in the standard compressed fragment, with generation guards and recovery-size limits.
  Other temporary documents require export before closing. Temporary short-link tokens stay in memory, and automatic file writes stay disabled.
  **Save current to library** reloads the portable document into the normal persistence path.
  Keep the mode and recovery failure visible. Verify library isolation, cross-profile URL recovery, quota fallback, and export behavior.
- **GitHub badge**: Share > **Copy for GitHub** copies a one-line `<picture>` link to the document's short link, with
  `/badge.svg` and `/badge-dark.svg` for GitHub's light and dark themes. The responder script draws both inline, with
  `textLength` fixing the width across fonts; GitHub fetches them through its image proxy, and the link's `#s=` key
  never reaches the server. **Copy threads for GitHub** copies the open threads as Markdown instead.
- **Link preview**: `og:image` is `https://{{MARS_HOST}}/og.jpg`, served by the responder script from the JPEG that
  `make tools-og ARGS="-g mars"` renders ([seo-og.md](seo-og.md#og-image-generation)). Re-render it after changing the
  logo or the registry description, then deploy.
- **Links**: full links are md.html-compatible (`#name=&md=`, plus `&o=<originId>`); folders use
  `#b=<state of a JSON bundle>`. Short links use the md.html scheme ([url-state.md](url-state.md#short-links-mdhtml-mars))
  with a different plaintext (`encodeState` bytes of the bundle JSON), in MARS's own KV, so `md.html` and MARS short
  links are not interchangeable. Client state: `localStorage["mars.shortLinks"]` and `["mars.shortTtl"]`.
  Share links also carry each document's reading width: `w` in full-link fragments and `readingWidth` in bundle documents.
  Received documents retain this width on their tree node, separate from the browser-wide preference.
  An explicit `w` fragment hint also supplies layout for legacy links. Invalid widths retain the recipient's preference.
  Verify actual text wrapping at the same viewport, plus reload and document switching, rather than only column ratios.

## Verify a change

For failures that a document export cannot reproduce, use [developer flags and diagnostics](../js/mars/README.md#developer-flags-and-diagnostics).
Keep its fields allowlisted. Report setup stages and fixed codes without document content, identifiers, URLs, credentials, or raw errors.
Register feature flags and report providers with the generic platform modules. Keep diagnostic controls in Developer tools, not feature dialogs.

Privacy and Credits use the app's 960px information-dialog cap. The shared viewport and height caps still apply on phones.
MARS bundles Plausible with URL redaction before transmission. Keep the legacy analytics script out of its HTML shell.
Read [Privacy and analytics](../js/mars/README.md#privacy-and-analytics) before changing analytics or privacy claims.

1. In `js/mars/`: `npm test` (codec, tree, merge, diff, match, and search logic) and `npm run typecheck`. `mars.spec.ts`
   never downloads a smart-search model. After changing `search/`, also run the search-quality evaluation
   (`npm --prefix eval run eval -- --check`, steps in the [js/mars README](../js/mars/README.md#evaluation)) and
   `MARS_BASE_URL=http://localhost:7373 make e2e-tools-test ARGS=mars-smart.spec.ts` (the built page with each model,
   cached in a local browser profile).
2. `npm run serve` builds the deploy-equivalent page and serves it at `http://localhost:7373/` and `/llms.txt`, with the
   responder script on an in-memory KV.
3. From the root: `MARS_BASE_URL=http://localhost:7373 make e2e-tools-test ARGS="mars.spec.ts mars-export.spec.ts mars-import.spec.ts mars-models.spec.ts mars-tables.spec.ts"`.
    After handoff changes, run `pdf.spec.ts` against the changed PDF Extractor page.
    Mutation specs use `_mars_test.ts` for authenticated isolation and automatic teardown, including manually created browser contexts.
    Production checks require an operator key in the Node process. Never send that key to the synthetic browser.
    Use [the cleanup commands](../js/mars/README.md#isolated-browser-test-data) for interrupted runs. Never clear user namespaces as test teardown.
4. Run steps 2, 4, and 5 of "Change a tool" in [`../AGENTS.md`](../AGENTS.md); for a viewer change, also the affected
   `md.spec.ts` cases.
5. After a deploy, `make e2e-tools-test ARGS=mars.spec.ts` (`MARS_BASE_URL` defaults to `https://mars.secutils.dev`).

Completion: steps 1-4 pass, step 5 passes after a deploy, and a viewer fix is present in both `viewer.js` and `md.html`.

## Native verification coverage

The completed parity and temporary-reading plans are retained in Git history, not as active plans.
On 2026-11-07, normal Safari saved an A4 portrait PDF and exact Markdown. Native keyboard Save and cancellation/focus-return checks passed.
Normal Edge saved A4 and Letter PDFs in both orientations, plus exact Markdown through its file picker.
These native PDFs contained readable document text without application chrome.

Native menu/split-view coverage and full rich-print combinations (Mermaid and colored models in every browser/settings combination) remain incomplete.
On 2026-11-08, two fresh native Firefox 159.0.1 profiles passed pairing, bidirectional Sync edits, reload, and private-library retention.
These checks used the native source editor and did not read existing Firefox profiles or share their credentials.
The user explicitly skipped the remaining VoiceOver checks on 2026-11-07. Engine assertions do not replace actual spoken-output checks.
For Android-specific selection changes, verify native Android Firefox selection in addition to emulated touch tests.
For shared table-selection defects, use desktop Firefox, Chromium, and WebKit, with touch toolbar checks.
