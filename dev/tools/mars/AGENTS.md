# MARS (`dev/tools/mars/`)

Read before changing MARS. Paths in this guide are relative to `dev/tools/mars/`.

MARS (Markdown Annotation & Review System) is the one own-host tool app ([platform.md](../shared/guides/platform.md#hosts-tools_host-mars_host)).
The prefix-match `_MARS` responder serves it at `/` of `{{MARS_HOST}}` (`mars.secutils.dev`).
The exact-match `_MARS_MD` responder serves `SKILL.md` at `/llms.txt`.
In-app URLs derive from `location.pathname`, so the page works on any host.
[README.md](README.md) describes user-visible behavior and the source layout.

## Guides

Read each guide whose trigger matches the change.

| Trigger | Read |
|---------|------|
| Grants, invitations, key epochs, the outbox, the responder, recovery packages, `#sync=` or `#live=` links | [src/sync/README.md](src/sync/README.md) |
| `src/search/`, `eval/`, a model, a dtype, or a Transformers.js upgrade | [src/search/README.md](src/search/README.md) |
| A privacy claim, analytics, or a new network request | [README: Privacy and analytics](README.md#privacy-and-analytics) and the Privacy dialog in `index.html` |
| Full links, bundles, short links, the `<state>` encoding | [url-state.md](../shared/guides/url-state.md#short-links-mdindexhtml-mars) |
| The `@su:responder-script` in `index.html`, hosts, bundles | [platform.md](../shared/guides/platform.md) |
| Shared tokens, primitives, glyphs, buttons | [design-system.md](../shared/guides/design-system.md) |
| A failure that a document export cannot reproduce | [README: Developer flags and diagnostics](README.md#developer-flags-and-diagnostics) |
| A browser test that writes to the service | [README: Isolated browser-test data](README.md#isolated-browser-test-data) |

## Code conventions

- **Format** with Prettier: `npm run format`. CI runs `npm run format:check`.
  `.prettierignore` excludes `src/doc/viewer.js`, because its shared sections stay byte-identical to Markdown Preview.
- **Module header**: each source file starts with a comment that states what the module owns and the invariants that callers depend on.
- **Doc comments**: each exported function, class, and type has a JSDoc comment that states its contract: units, failure behavior, and side effects.
  A comment explains a reason or an invariant. The code states the mechanics.
- **Named constants**: limits, time-to-live values, sizes, and storage keys are module-level constants with their unit in the name or the comment.
- **Errors**: user-visible failures carry a fixed code and a plain sentence. Diagnostics take allowlisted fields only.
- **Dependencies**: `package.json` pins exact versions. A bundled dependency needs a Credits entry in `index.html` and its license notice in the build.

## Invariants

### Source layout and build

- `index.html` is a thin shell: SEO head, static DOM, favicons, the `@su:responder-script`, and two `gzip-base64` bundles.
  The bundles are `mars` (this app, built by esbuild) and `three-mf` (`../shared/libs/three-mf/`, shared with `../md/index.html`).
- The shell imports the `mars` bundle at startup. `html.mars-boot` hides the body until the bundle injects its CSS.
- `sync-assets.ts` assembles the responder script for local preview, deployment, and the sandbox test. Keep one assembly function.
- CodeMirror builds into one embedded module graph and loads lazily from a Blob URL. Mermaid and Paged.js load on demand from a CDN.
- The page is cross-origin isolated (COOP `same-origin`, COEP `credentialless`) so that model threads work.
  A new cross-origin embed must load without credentials.

### Viewer parity with Markdown Preview

- `src/doc/viewer.js` is a copy of the live script in `../md/index.html`, without its output UI and fragment autosave.
  `viewer.d.ts` is its host API.
- A fix to rendering, block editing, formatting, or comments lands in both files.
  After the port, diff the shared sections. `test/viewer-parity.test.ts` compares several of them byte for byte.
- MARS-only code stays out of `../md/index.html`: the "Review changes hooks" section
  (`onRendered`, `setThreadHooks`, `blockText`, `requote`), the table hooks, the sync hooks, and the sanitizer calls.
- `src/doc/diagrams.js` supplies maps and models to the viewer and to the standalone export runtimes.

### Storage

- Documents live in IndexedDB database `mars` with stores `nodes`, `docs`, and `meta`.
- Per-document side data lives in `docs` under `<kind>:<id>` (`db.getSide`): `base` (review baseline), `opened` (source text as received),
  `vec` (search index), and `sync` (replica, outbox, keys, progress). A new kind needs no new store.
- Open the database without a requested version. Some browsers hold version 2 from a withdrawn feature.
  Keep unknown stores. Never downgrade or delete the database.
- Every document has an `originId` that survives sharing. A link for a known `originId` merges threads by comment `id`, or by `author` plus `at`.
- The fragment is a route (`#doc=<id>`) or an incoming share. Client preferences live in `localStorage` under `mars.*` keys.

### Incoming documents

- Open, drops, imports, Load URL, and `?url=` share `src/io/source.ts` for source extraction and naming.
  An explicit name wins over a frontmatter title, a Markdown heading, and the file name.
- `src/io/input.ts` owns input generations and abort signals. Pass the current operation through each receive and navigation step.
  Check it after each asynchronous read and before you select or change a document.
- `share.updateExisting` matches an incoming copy by origin, by file handle (`isSameEntry`), or by content after the reader confirms (`share/match.ts`).
  It decides the text against `opened`: source changed takes it, stored copy changed keeps it, both changed asks (default **Save as a copy**).
- An outside change keeps the previous text as the `base` side record (`app.rebase`) until **Mark reviewed**.
- Converted HTML never keeps a writable file handle. Unchanged Markdown and review blocks keep their original bytes on download.
- Load URL and `?url=` both call `share.receiveUrl()`. The browser fetches the source directly, without a proxy.
- PDF Extractor hands over Text and Markdown through a full link with an explicit name.
  The exact text stays in the fragment. No PDF bytes leave the browser.
- Linked files exist in Chromium only (File System Access pickers). Plan no Firefox or Safari port of the 2-second poll.

### Links

- Full links are Markdown Preview-compatible: `#name=&md=`, plus `&o=<originId>` and optional `&w=<width>`.
  Folders use `#b=<state of a JSON bundle>` with `readingWidth` on each document.
- Short links use the Markdown Preview scheme with a different plaintext, in the MARS key-value store.
  MARS and Markdown Preview short links are not interchangeable.
- `t=1` in the fragment or `?temporary=1` selects the memory backend before the database opens.
  Denied storage and quota failures select it too. A temporary session never reads or writes the library or its preferences.
- `share/temporary.ts` keeps only the current document in the fragment, with generation guards and size limits.
  Temporary short-link deletion tokens stay in memory, and automatic file writes stay off.
  Keep the mode and a recovery failure visible in the header and the banner.
- A link carries the reading width: `w` in a full link, `readingWidth` in a bundle document.
  A received document stores it on its tree node, separate from the browser preference. An invalid width keeps the preference.
- **Copy for GitHub** links `/badge.svg` and `/badge-dark.svg`, which the responder script draws inline.
  `textLength` fixes the width across fonts. The `#s=` key of the link never reaches the server.

### Sync and Live links

- `src/sync/` owns Sync and Live links. Its hooks stay out of Markdown Preview.
  `index.ts` is the light entry. It loads `manager.ts`, a facade over the modules in `manager/`, only for connected content.
  The modules share state through `manager/state.ts`. Keep top-level code in them free of calls into sibling modules: they import each other in cycles.
- `db.commitReplica` saves the Markdown projection, replica state, outbox, and received progress in one transaction.
- A protocol-only commit keeps the document timestamp and the search index entry.
  Library promotion moves the vector record of the selected document and leaves other index entries valid.
- Connection metadata is scoped by collection and member. A Live grant stays separate from the private connection of the owner in the same profile.
- Sync and Live links need a wrapping key that differs from the bearer token. Reject a link without one before any request.
- Treat each service response as untrusted input. Validate its shape, and verify it against the owner key from the link, before use.
- A downloaded checkpoint that fails local verification throws `RejectedPacket`. The receiver skips that checkpoint and keeps the connection.
- A live fragment selects memory storage before IndexedDB opens. **Save live document to library** is one atomic promotion.
- Transport keys, credentials, and outbox records never enter Markdown, HTML, PDF, ZIP, or agent handoffs.
  Only the optional encrypted recovery file carries them.
- Remote edits never write to a linked file. A connected document needs an explicit disk **Save**.

### HTML safety

Every document, comment, and review baseline can come from another person, and the page holds the library and its connection keys.

- Document-derived HTML reaches `innerHTML` only through `src/doc/preview-safety.ts`:
  `safeRawHtml` for each raw HTML token, then `safeRenderedHtml` for the complete renderer output. This applies to the preview, comments, the review-changes baseline, and exports.
- `safeRawHtml` removes `class`, `id`, and `data-*`, so a document cannot forge the attributes that the renderer adds afterwards (source ranges, block markers, table layout).
- A renderer that escapes or sanitizes raw HTML also escapes raw-block text (`escapeRawBlockText` in `viewer.js`).
  marked emits the text inside `<code>`, `<kbd>`, `<pre>`, and `<script>` tags unescaped.
- Dialog markup escapes each interpolated value with `esc()`, including values from the service and from a recovery file.
- Look up an app element by a selector that a heading id cannot match, for example `script[data-su-bundle="three-mf"]`.
  Generated heading ids are document content.

### Tables

- `src/doc/table*.ts` owns source operations, proportional layout, and Preview controls. Table editing is MARS-only by user decision.
- Column proportions live in an adjacent `su-table` comment. Use no side record and no new link format for them.
- A cell edit splices only the selected content. An alignment edit changes only the divider colons.
  Selection formatting in Preview uses the source range of the selected cell.
- The shared block mapper keeps byte parity with Markdown Preview apart from the explicit table hook calls.
  Advance table ranges past source-mapped code and HTML tokens, inline list fences included, before the search for a table.
- A whole-table move or deletion takes the `su-table` comment with it. A column move keeps cell Markdown, alignment, and saved widths.
- Apply a cell draft before a view change, a share, or an export. Discard a stale draft after an outside source change.
- Preview and export use the same layout helper. Add controls only after the live Preview renders.
  Cancel pointer gestures on source changes, navigation, Source mode, and export capture.

### Export

- `src/export/` captures one immutable document and uses one export-safe render for HTML and PDF.
  The HTML preview and the download are the same output. The PDF preview and the print job are the same prepared pages.
- The reader and print runtimes compile separately into inline source strings. Paged.js loads only for PDF pagination.
- Draft settings stay separate from document state. An output action commits them to `mars.exportSettings`.
- Preview frames have opaque origins. The message bridge checks the source window and the operation identifier.
- Keep print CSS compatible with the Paged.js parser: expand heading selectors, with no comma lists inside `:is()`.
  Add screen preview scaling after pagination.

### Analytics and diagnostics

- MARS bundles the Plausible tracker with URL redaction in `src/platform/analytics.ts`. Keep redaction in the browser.
  Keep the legacy `/js/script.js` tracker out of the shell.
- A debug report takes allowlisted fields: setup stages and fixed codes.
  It excludes document content, identifiers, URLs, credentials, and raw errors.
- Register flags and report providers with `src/platform/`. Diagnostic controls live in **Developer tools**, not in feature dialogs.

### UI

- MARS uses the EUI look: Borealis tokens, EUI icons, and a header with a docked sidebar.
  Privacy and Credits sit in the sidebar. MARS has no CTA banner and no footer.
- The brand tokens (`--mars-*`, `--brand*`) live in `../shared/themes/mars.css` and follow the logo in `assets/`.
  The header mark and the data-URI favicons in `index.html` are copies of that artwork.
- Sidebar documents are real anchors to `#doc=<id>`. The action button and the rename input stay outside the anchor.
  Pointer navigation flushes pending edits and defers tree replacement until the gesture ends.
- **Copy link** is the one dialog for snapshot and Live links. It defaults to Snapshot and Full.
  Expiry choices are 1, 7, 14, and 30 days, without calendar dates. Source-side **Live links** management sits in the dialog footer.
  The generated URL replaces the action instruction without a change to the height of the result area.
- The Privacy and Credits dialogs use the 960px information-dialog cap. Link-security explanations belong in Privacy.
- The link preview image is `/og.jpg`, served by the responder script. After a logo or description change,
  render it again with `make tools-og ARGS="-g mars"`, then deploy.

## Verify a change

1. Run `npm run format:check`, `npm run typecheck`, and `npm test`.
2. For a change in `src/search/`, run `npm --prefix eval run eval -- --check`.
3. Run `npm run serve`. It serves the deploy-equivalent page at `http://localhost:7373/`.
4. From the repository root, run the browser specs that cover the change:
   `MARS_BASE_URL=http://localhost:7373 make e2e-tools-test ARGS="--project=chromium <specs>"`.

   | Change | Specs |
   |--------|-------|
   | Any | `mars.spec.ts` |
   | Export | `mars-export.spec.ts`, `mars-models.spec.ts` |
   | Imports, source naming | `mars-import.spec.ts` |
   | Tables | `mars-tables.spec.ts` |
   | Temporary sessions | `mars-temporary.spec.ts` |
   | Share dialog, Privacy dialog, analytics | `mars-sharing.spec.ts` |
   | Sync, Live links, responder | `mars-sync.spec.ts`, `mars-sync-ux.spec.ts`, `mars-sync-followup.spec.ts`, `mars-cleanup.spec.ts` |
   | Review markers | `mars-review-markers.spec.ts` |
   | Smart search | `mars-smart.spec.ts` (downloads each model once. No other spec downloads a model.) |
   | Viewer | The affected `md.spec.ts` cases |
   | PDF Extractor handoff | `pdf.spec.ts` |

5. For a selection, toolbar, or table-handle change, run the same specs with `--project=firefox` and `--project=webkit`.
6. Run steps 2, 4, and 5 of "Change a tool" in [`../AGENTS.md`](../AGENTS.md).
7. After a deploy, run `make e2e-tools-test ARGS=mars.spec.ts` (`MARS_BASE_URL` defaults to `https://mars.secutils.dev`).

Completion: steps 1-6 pass, step 7 passes after a deploy, a viewer fix is present in both `viewer.js` and `../md/index.html`,
and README.md, SKILL.md, and the Privacy dialog describe the changed behavior.
Report each step that you did not run as unverified.

Mutation specs use `_mars_test.ts` for isolated test scopes. A production run needs the operator key in the Node process only.
Never send that key to the browser, and never clear user namespaces as test teardown.

## Verification gaps

Browser engines in Playwright do not cover these checks. Do them by hand when a change touches the area:

- Native print dialogs and saved PDF files, for each paper size and orientation.
- Native browser link menus and split view for sidebar documents.
- Rich print combinations: Mermaid and colored models in every browser.
- Screen-reader output (VoiceOver). Engine assertions do not replace it.
- Text selection in native Android Firefox, for selection changes.
