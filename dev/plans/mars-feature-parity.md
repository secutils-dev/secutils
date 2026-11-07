# MARS useful-feature parity plan

Date: 2026-11-05
Status: Phase 0 implemented, deployed, and checked. Manual browser-chrome checks remain pending.
Phase 1 is implemented and deployed. Manual system print/save checks remain pending.
Phase 2 is implemented, committed, deployed, and checked.
Phase 3 is deployed, and its application checks passed. Manual browser gates and two unrelated registry failures remain pending.

## Goal and scope

MARS will cover the useful reading, editing, review, and export features of Markdown Preview.
The implementation will retain the MARS document library and its review workflow.
Phase 0 adds an optional native browser menu without replacing the current custom menu.
Phase 1 adds HTML and PDF output through a compact export workspace.

The user explicitly excludes compatibility work from this plan.
Legacy redirects, short-link migration, and preservation of old endpoints are outside the scope.
Fragment autosave, a temporary-document mode, and private-window access to stored documents are also outside the scope.
Removal of the existing Markdown Preview implementation requires a separate decision.
Deployment of later phases requires a separate request.

## Existing foundation

The rendering code in both apps matches across 1,126 lines.
Block operations and selection formatting also match.
MARS already supports Mermaid, math, alerts, maps, STL, colored 3MF, preview edits, source edits, and review comments.
The existing model importer also supports ZIP64, worker cancellation, and bounded geometry.

MARS lacks the HTML/PDF output pipeline and the exported HTML view.
Its URL form and `?url=` entry point use different import paths.
The query entry point does not extract Markdown from exported HTML or honor frontmatter titles for naming.

Initial research checks:

- MARS unit tests: 28 passed.
- MARS type checking: passed.
- Isolated execution of `receiveUrl()`: reproduced the exported-HTML and title differences.

## Implementation references

All source paths in this document are relative to the Secutils repository root.

| Surface | Existing source |
| --- | --- |
| Tool contracts and browser policy | `dev/tools/AGENTS.md` |
| MARS architecture and checks | `dev/tools/guides/mars.md` |
| Shared visual primitives | `dev/tools/guides/design-system.md` |
| Dialog and popover behavior | `dev/tools/guides/components.md` |
| Export conventions | `dev/tools/guides/chrome.md` |
| Long tasks and busy states | `dev/tools/guides/javascript.md` |
| Static shell and toolbar | `dev/tools/mars.html` |
| Tree markup and input handlers | `dev/tools/js/mars/src/sidebar/sidebar.ts` |
| Document routes and persistence | `dev/tools/js/mars/src/app.ts` |
| Startup and route handling | `dev/tools/js/mars/src/main.ts`, `src/share/share.ts` within the MARS package |
| Renderer and editing API | `dev/tools/js/mars/src/doc/viewer.js`, `viewer.d.ts` |
| Shared modal and clipboard helpers | `dev/tools/js/mars/src/ui/ui.ts` |
| App layout | `dev/tools/js/mars/src/ui/components.css` |
| Export reference implementation | `dev/tools/md.html`, output handlers and output-generation section |
| Behavior reference | `dev/tools/md.skill.md` |
| Browser fixtures and regression cases | `e2e/tools/md.spec.ts`, `mars.spec.ts`, and shared Markdown helpers |
| Build and local server | `dev/tools/js/mars/build.mjs`, `serve.mjs` |

## Phase 0: Native browser menu through a modifier

### User decision

The custom context menu remains the default.
A modifier gesture exposes the native browser link menu on documents.
Private windows do not need access to the original document.
The primary use cases are another normal tab, another normal window, and browser split view.

### Implemented interaction

| Interaction | Result |
| --- | --- |
| Ordinary right-click on a document | Current MARS custom menu |
| Shift + right-click on a document link | Native browser link menu |
| Click the document name | Open the document in the current MARS tab |
| Ctrl/Cmd + click or middle-click | Browser opens the local document link according to its settings |
| Shift + click | Browser handles its normal new-window gesture |
| Click `⋮` | Current MARS custom menu |
| Right-click a folder | Current MARS custom menu |

Firefox already bypasses `contextmenu` handlers for Shift + right-click.
Other browsers need a modifier branch that leaves the event uncanceled.
A real anchor supplies the native link actions.
Removing `preventDefault()` from a generic row alone only exposes a generic page menu.

The browser owns the native menu entries.
MARS cannot add a split-view entry or force a private-window entry to appear.
JavaScript cannot reliably open the native menu through a synthetic event or a custom menu button.

### Implementation

Documents and search results contain real anchors to `#doc=<id>`.
The document name, icon, and practical row hit area belong to the anchor.
The `⋮` button and rename input stay outside it.
Folder controls retain their existing behavior.

Ordinary right-click still opens the custom menu.
Shift + right-click on a document anchor leaves the native event uncanceled.
Unmodified primary activation uses in-app navigation.
Modified clicks and middle-click retain native navigation without selecting another document in the source tab.

The row retains tree semantics and selection metadata.
The document link is the keyboard focus target.
Arrow navigation, Home, End, Enter, Space, F2, deletion, and the custom keyboard menu remain available.
Focus survives tree refreshes, and each tree has one tabbable entry.

Document custom menus include the hint `Shift + right-click for browser actions`.
The hint is explanatory text, not a button.
It is hidden for devices without hover.
Folder menus do not include the hint.

### Persistence fix

The initial browser test reproduced stale text after immediate native navigation.
The source editor autosaves after 500ms, but another tab can read the document before that write.

Pointer navigation now starts `viewer.flush()` before the new tab reads the document.
The tree defers replacement of its anchor until the pointer gesture finishes.
Pointer release, cancellation, window blur, drag completion, and drop release that deferral.
This also preserves WebKit dragging, which does not always deliver pointer release during a native drag.

The document link needs no URL compression, portable snapshot, server request, or storage-schema change.
The phase does not add cross-tab live editing or change the conflict model.

### Checks and deployment record

Deployment command:

```bash
make deploy-tools ARGS=mars
```

The deployment succeeded at <https://mars.secutils.dev/>.
It published the MARS page, its companion skill, and four tools-host discovery aggregates.
The deployed application bundle matches the checked local build byte-for-byte.
Its SHA-256 is `11904c24fc97827c4ea905fd329ccf079ec79737ec0a73a770c3c6e268b4473a`.

Local checks:

- MARS unit tests: 30 passed, including local-link and label-escaping tests.
- MARS type checking and deploy-equivalent preview build: passed.
- `make tools-verify ARGS=dev/tools/mars.html`: passed.
- `make tools-check`: passed for repository metadata and README links.
- Marketing-site synchronization: skipped because `SECUTILS_TOOLS_PROMO_HOME_INDEX` is not set.
- Responder smoke suite: 20 passed.
- Shared-asset unit tests: 6 passed.
- Shared ECHO/Webhook UI suites: 66 passed across Chromium, Firefox, and WebKit.
- E2E lint for `mars.spec.ts` and `playwright.tools.config.ts`: passed.
- Full local MARS browser suite: 111 passed before the extra phone case.
- Final Phase 0 browser cases: 12 passed across Chromium, Firefox, and WebKit.
- Installed Chrome 156.0.8078.80: custom menu, Shift bypass, and middle-click document route passed.
- Post-deploy full MARS browser suite: 114 passed across Chromium, Firefox, and WebKit.

The Playwright engines were Chromium 156.0.8078.4, Firefox 157.0, and WebKit 27.2.
The WebKit host navigates the current page for modified clicks, even with an ordinary test anchor.
Its tests check uncanceled events, same-profile route access, recent-edit persistence, keyboard navigation, dragging, and touch actions.
Chromium and Firefox tests exercise actual new-tab and new-window gestures.

The OpenCode browser also displayed the custom-menu hint and passed the Shift-right-click bypass check.
Native browser-chrome menu entries are not exposed to DOM assertions.
Manual Safari and Edge menu contents, split-view commands, and assistive-technology interaction remain unverified.
Private-window document access is not an acceptance criterion.
Shift + F10 is not documented as a native-menu shortcut.

Implementation is complete and deployed.
The manual browser-chrome checks remain explicit follow-up checks, not claimed successes.

## Phase 1: HTML/PDF pipeline and export workspace

### UX decision

Add one outlined `Export…` control beside the existing primary `Share` control.
Keep `Preview | Source` as the main view switch.
Keep the document gear limited to reading, editing, linked files, and review changes.
HTML and PDF settings belong to the export workspace, not the document gear.

Clicking `Export…` opens one native modal dialog.
The dialog contains `HTML | PDF` tabs, format-specific settings, and an output preview.
It also provides a direct Markdown download without settings.
This design avoids another permanent HTML view segment, separate PDF/HTML toolbar buttons, and an export-menu-to-settings-modal sequence.

Suggested toolbar order:

```text
Open   Preview | Source   Comments   Find   Settings   Export…   Share   Fullscreen
```

The existing Share menu keeps links, review handoffs, and short-link actions.
Move its `Download .md` action into the export workspace.
Keep direct Markdown download in document action menus.
Add `Export…` to document action menus for the selected document.
Keep folder ZIP export and folder export to disk unchanged.

### Desktop layout

```text
┌ Export “Document name”                                [Close] ┐
├─────────────────────────┬───────────────────────────────────┤
│     [ HTML ] [ PDF ]     │ Output preview                    │
│ Format-specific settings│                                   │
│ All settings visible    │ Rendered document or PDF pages    │
│                         │                                   │
├─────────────────────────┴───────────────────────────────────┤
│ Download Markdown       Close   Copy HTML   Download HTML   │
└─────────────────────────────────────────────────────────────┘
```

The PDF tab replaces the HTML actions with `Print / Save PDF`.
The browser print dialog supplies Save as PDF and printer selection.
MARS does not claim that it downloads PDF bytes directly or knows whether the user saved the file.

Use a dialog that occupies most of the viewport, with a compact settings column and a flexible preview column.
Center the format tabs above the settings column.
Use the document name in the main heading, without a separate name subtitle.
Keep the action footer visible while the dialog body scrolls.
Use shared tokens, fields, buttons, tabs, and dialog styling.
Use a dedicated export dialog instead of expanding the small generic confirmation modal.

Use a named native `<dialog>` with native focus containment and dismissal.
Give the format tabs correct tab roles, selection state, and arrow-key navigation.
Keep inactive settings and previews outside the focus sequence.
Use a stable toolbar button or document entry as the opener, including actions from a custom menu.
Keep Export disabled for an empty document.
Before opening the dialog from document fullscreen, establish a visible dialog path or exit fullscreen.

### Phone and narrow layouts

1. Stack settings before the preview at narrow widths.
2. Keep the format tabs and primary action visible.
3. Keep the preview inside a bounded scroll region.
4. Scale PDF pages visually without changing their print dimensions.
5. Keep native page zoom available.
6. Check 320px, 390px, 768px, and desktop widths with both themes.

Collapse the toolbar's Export label to its icon only where the existing toolbar requires it.
Keep an accessible name on the control.
Avoid horizontal scrolling as the normal way to reach export actions.

### HTML settings

List all five HTML settings together, without a disclosure section.

| Setting | Default | Placement |
| --- | --- | --- |
| Reading width | Current MARS reading width | Main settings |
| Table of contents | On | Main settings |
| In-page find | On | Main settings |
| PDF action in exported HTML | On | Main settings |
| Embed Markdown source | Off | Main settings |

Reading width retains Comfortable, Wide, Full width, and Custom.
The custom range remains 320–2400px.
An export-width change does not change the MARS reading width.
The embedded PDF action uses the print settings from the PDF tab.

Theme, progress, back-to-top, and code copying remain built-in reader features.
These features do not need new settings.
The table of contents appears only when the document has useful headings.
Disabled or unavailable features need a short explanation rather than dead controls.

The source setting describes the source as readable content inside the HTML file.
Review comments remain excluded from HTML/PDF output, including embedded source.
Phase 1 preserves body-only embedded source from Markdown Preview.
Do not describe that representation as a lossless backup of frontmatter and review metadata.

### PDF settings

The PDF tab exposes two compact controls:

- Paper size: A4 or Letter, with A4 as the default.
- Orientation: Portrait or Landscape, with Portrait as the default.

Letter and Landscape are small proposed UX additions, not existing Markdown Preview settings.
They must fit the same pipeline and receive page-dimension tests.
Custom paper sizes, margin sliders, cover pages, and typography editors are outside the scope.

Keep readable fixed margins, page numbers, and document-title footers from the existing print output.
Use light print styling independently of the MARS theme.
Retain vector text, complete Mermaid SVGs, and color model snapshots.
Exclude interactive reader controls and review marks from the printed pages.

### Settings and action behavior

1. Restore the last committed HTML and PDF settings from browser storage.
2. On first use, seed HTML width from the current reading width.
3. Keep draft settings separate from document state.
4. Update the active preview after a valid settings change.
5. Preserve draft settings across format switches within the dialog.
6. Commit the settings after a download, copy, or print request.
7. On cancellation, discard draft changes without changing committed settings.

Use one versioned browser-storage value, for example `mars.exportSettings`.
Handle malformed values with the defaults.
Export settings do not change Markdown, review threads, share links, or the IndexedDB schema.

`Ctrl/Cmd+P` opens this dialog directly on the PDF tab for the current document.
The shortcut does not print the MARS sidebar or app chrome.
Keep ordinary browser printing available inside exported standalone HTML.

The HTML preview is the actual standalone output, not a copy of the editable MARS DOM.
The PDF preview contains the actual paginated print document.
Downloads and printing use the prepared output that the preview displays.
Expensive model processing and pagination do not restart solely because the user presses the output action.

### Output pipeline structure

Create a dedicated `src/export/` area in the MARS package.
Keep rendering, document storage, and export orchestration separate.

| Module | Responsibility |
| --- | --- |
| `settings.ts` | Typed defaults, stored values, and format-specific settings |
| `snapshot.ts` | Immutable document input and render-cache keys |
| `dialog.ts` | Export dialog state, tabs, drafts, previews, and actions |
| `html.ts` | Standalone HTML assembly |
| `pdf.ts` | Print document assembly, pagination, and print requests |
| `reader-runtime.ts` | Standalone HTML navigation, controls, and optional PDF action |
| `print-runtime.ts` | Pagination, readiness messages, and printing |
| `export.css` | Export workspace layout, separate from document styling |

1. Add a narrow viewer API to commit a block edit and capture an export snapshot.
2. Flush the current document before capture.
3. Capture the name, body, requested settings, and saved model views once.
4. Render the snapshot through the existing export-safe renderer.
5. Keep review highlights, diff marks, block handles, and source-map attributes out of output.
6. Capture non-current documents from storage without changing the current selection.
7. Share rendered content between HTML and PDF preparations for the same snapshot.

The renderer needs an explicit export API rather than an export module that reads viewer globals.
The new API must not call stateful source parsing that resets the current document's comments or name.
The default export filename uses the MARS document name and the existing filename sanitizer.
Escape document titles before insertion into HTML and print metadata.

### Standalone output and bundling

1. Port the useful output CSS, HTML assembly, and print CSS from `md.html`.
2. Reuse the existing map/model runtime through explicit module boundaries.
3. Bundle standalone reader and print runtimes as source strings during the MARS build.
4. Inline those strings into the generated document.
5. Keep the deployed MARS bundle self-contained and compatible with the existing inliner.

Do not copy the entire live MARS app into an exported document.
Do not depend on stringified bundled functions with unresolved closures or minifier-renamed external references.
Avoid scraping global stylesheets to collect export styles.
Use explicit export styles with a known dependency set.

Standalone HTML retains interactive map geometry, STL/3MF views, code copying, reader width, theme, find, and source extraction.
Mermaid exports retain complete static SVGs rather than the current zoomed viewport.
Model fences retain their saved view metadata.
Unsaved viewer navigation stays temporary, as it does today.

Exports contain no document upload, analytics script, library storage, or model-download code.
Map basemap tiles remain network-dependent.
External images, fonts, KaTeX styles/fonts, and optional PDF pagination retain explicit network caveats.
The baseline does not require inlining every remote resource.

### Preview isolation and PDF readiness

Use sandboxed iframes for the HTML preview and the print document.
The HTML preview must not access the MARS IndexedDB, application DOM, or document library.
Avoid the combination of `allow-scripts` and `allow-same-origin` for user-controlled `srcdoc`.
Allow only the capabilities required for reader controls, links, downloads, and printing.

Opaque iframe origins require a small message bridge instead of direct access to child globals.
Messages carry an operation identifier and no document-library command.
The parent accepts messages only from the current iframe window and operation.
The child accepts control messages only from its parent.
Sandboxed `srcdoc` can require `targetOrigin="*"`, so source-window checks remain mandatory.

1. Prototype sandboxed Paged.js preparation and printing before the complete UI integration.
2. Use explicit pagination completion rather than treating a timeout as readiness.
3. Wait for required images, fonts, and model print snapshots before pagination completes.
4. Prepare PDF pages only after selection of the PDF tab or an output action that needs them.
5. Keep `Print / Save PDF` disabled until the prepared document is ready.
6. Start the print action from the user's explicit click.
7. Release frames, object URLs, event listeners, and caches after closure or replacement.

If a browser blocks the parent-triggered print command, investigate an explicit print control inside the prepared iframe.
Do not remove isolation solely to make printing work.
An untested unsandboxed iframe is not an acceptable fallback.
The prototype gate must establish a working and isolated print path for the supported desktop browsers.

### Busy states and cancellation

The dialog first displays `Preparing preview…` without starting an output action.
Use a generation identifier for every document, format, and settings change.
Ignore results from superseded operations.
Stop superseded workers or frames where the existing engine permits cancellation.

Keep Close available during preparation.
Keep output actions disabled until their output is ready.
Treat invalid width input as a field error, not as a global failure.
On a preparation failure, display a visible error and a Retry action.
On a missing PDF dependency, keep HTML and Markdown download available.

The modal holds an immutable document snapshot.
If linked-file reload changes the document, display a contextual `Document changed` message with `Refresh preview`.
Refreshing captures a new snapshot and supersedes the old preparation.
The export must never switch to a different document silently.

After HTML download or copying, display the existing toast pattern.
After a print request, use print-specific feedback without claiming that a PDF file was saved.

### Phase 1 completion gate

- The toolbar gains one Export control and no additional permanent format toggles.
- HTML and PDF settings remain separate from the document gear.
- All HTML settings remain directly available.
- HTML preview and downloaded HTML match the same prepared document.
- PDF preview and print output use the same page preparation.
- HTML, Copy HTML, PDF, and Markdown actions work for the current document.
- A document action menu can export another document without changing the active document.
- Exported maps/models remain interactive without external renderer requests.
- PDF retains vector text, complete Mermaid diagrams, and colored model images.
- Review threads, review marks, and app storage do not leak into the output.
- Cancellation and superseded preparations cannot trigger stale downloads or print requests.
- Desktop, phone, both themes, keyboard focus, and reduced-motion checks pass.
- Export from document fullscreen remains visible and usable.

### Phase 1 implementation record (2026-11-06)

The implementation adds one Export control and a dedicated native dialog.
The dialog uses 94% of the desktop viewport, with centered format tabs above the settings column.
All HTML settings appear together, and the document name appears only in the main heading.
PDF pages intentionally remain white in both MARS themes.
HTML, Copy HTML, PDF preparation, and Markdown download use an immutable document snapshot.
Document action menus can export a non-current document without changing the selection.
Settings drafts commit to `mars.exportSettings` after an output action.
The Share menu no longer contains Markdown download.
Folder exports and the IndexedDB schema remain unchanged.

The new `src/export/` modules own the workspace and standalone output.
`src/doc/diagrams.js` supplies the shared map/model runtime through an explicit module boundary.
The build compiles standalone reader and print runtimes to inline source strings.
HTML needs no additional external export library.
Paged.js 0.4.3 loads only when PDF pagination is needed, including through the PDF action in standalone HTML.
Later preparations use the browser HTTP cache.
The HTML preview does not fetch Paged.js.

Both previews use opaque sandboxed frames without `allow-same-origin`.
Their message bridge checks the current source window and operation identifier.
PDF readiness follows explicit pagination completion after required images and fonts are ready.
Print actions reuse prepared pages and do not repeat model processing or pagination.
The initial print frame also provided a directly activated print control.
The export UX follow-up removed that duplicate control.
Screen scaling runs after Paged.js processes the print styles and is disabled for printing.
Cancellation disposes frames and listeners, stops export model workers, and ignores superseded results.

Local checks:

- MARS unit tests: 36 passed.
- Type checking, build, and deploy-equivalent preview build: passed.
- Full MARS and export browser suites: 169 passed, 2 skipped across Chromium, Firefox, and WebKit.
- Final export-only suite: 58 passed, 2 skipped after the standalone-reader PDF and print-media checks were added.
- After the export workspace UX review: full suites passed 172 cases, with the same 2 GPU cases skipped.
- The skipped cases require Chromium's software GPU for color-pixel assertions.
- Offline map and STL files, complete Mermaid SVGs, and sandboxed PDF pages passed in all three engines.
- Saved 3MF views and color PNGs passed in Chromium, including actual offline HTML files.
- A4, Letter, Portrait, and Landscape page-dimension checks passed in all three engines.
- Shared ECHO/Webhook UI suites: 66 passed across the three engines.
- Responder smoke tests: 20 passed. Shared-asset unit tests: 6 passed.
- Affected Markdown Preview rendering/export cases: 8 passed.
- Shared reading-width cases and the Markdown Preview width/export case: 10 passed across Chromium and Firefox.
- E2E lint, tools verification, and repository tools checks: passed.
- Marketing-site synchronization: skipped because `SECUTILS_TOOLS_PROMO_HOME_INDEX` is not set.

The initial full run found ambiguous test locators for reading-width fields.
The shared helper now scopes them to the document options popover.
The full suite passed after this change.

Automated print cases intercept the print request instead of opening a system dialog.
Real print/save output still needs manual checks in supported desktop browsers.
Searchable PDF text, saved page dimensions, page numbers, diagram completeness, and model colors remain part of that manual gate.
Deployment (2026-11-06):

```bash
make deploy-tools ARGS=mars
```

The deployment published MARS, its companion skill, and the four tools-host discovery aggregates.
The application is live at <https://mars.secutils.dev/>.
The deployed MARS bundle matches the checked local build byte-for-byte.
Its SHA-256 is `f44008ad15af30f7bc1a75b5f70e94ded17386f68c99c6019c5b28ba12924bda`.
The live response retains COOP `same-origin` and COEP `credentialless`.
Post-deploy browser checks passed: 172 cases across Chromium, Firefox, and WebKit, with the same 2 GPU cases skipped.

```bash
make e2e-tools-test ARGS="mars.spec.ts mars-export.spec.ts --workers=3"
```

URL-import unification, PDF Extractor handoffs, and retirement work remain outside Phase 1.

### Export UX follow-up (2026-11-07)

The settings omit the generic table-of-contents hint and the explanation about PDF-tab settings.
The later follow-up also removes the conditional hint for documents with too few section headings.
The dialog no longer contains a Reset settings button.
Saved settings and cancellation behavior remain unchanged.

PDF pages appear centered in the preview, including after viewport and scrollbar changes.
Screen scaling does not change the print dimensions or print layout.
The dialog footer provides the only Print / Save PDF button.
The print frame remains isolated and accepts the existing guarded print messages.

The status appears during preview preparation or after a failure.
The status disappears when the preview is ready, and the output action becomes available.
PDF preparation still waits for pagination, images, and fonts.
Failures retain Retry and the other export formats.

The initial export-only browser run passed 67 cases, with 2 GPU-specific cases skipped.
The final local regression gate passed 250 cases, with the same 2 GPU-specific cases skipped.
The new viewport/scrollbar case also passed in Chromium, Firefox, and WebKit.
Unit tests passed 43 cases. Type checking, builds, lint, tools checks, and diff checks passed.
The phase 2 deployment includes these follow-up changes.

## Phase 2: Consistent URL import and source reopening

This phase closes useful import gaps. It does not preserve legacy URLs or add redirects.

1. Create a shared, state-independent source extraction helper.
2. Use it for Open, drop, Load URL, and `?url=`.
3. Route both URL entry points through the same document-receive path.
4. Preserve stable URL origins and existing incoming-copy conflict choices.
5. Honor explicit incoming names first.
6. Without an explicit name, use frontmatter title, then a heading, then the filename or fallback.
7. Strip HTML and Markdown filename extensions consistently.
8. Preserve CRLF and valid review blocks through ordinary Markdown imports and downloads.

Exported HTML with embedded source reopens as editable Markdown.
Exported HTML without embedded source retains the current raw-HTML import behavior.
The application does not infer a lossless Markdown source from rendered HTML.
Phase 1's body-only source does not restore omitted review metadata or frontmatter.

Use generation guards for overlapping URL loads.
Opening a different file, document, or URL cancels or supersedes an earlier load.
Network and CORS failures retain the current document and display a useful error.
Fetching remains a browser operation, not an API proxy request.

Completion requires equivalent content and naming across the form and query entry points.
Completion also requires exported-source reopening through file Open, drop, and URL import.
Reopening must not duplicate a known source or silently replace locally edited text.

### Phase 2 implementation record (2026-11-07)

`src/io/source.ts` provides state-independent source extraction and naming.
Open, preview drops, library imports, Load URL, and `?url=` use this helper.
Explicit names precede frontmatter titles, Markdown headings, and filename fallbacks.
ZIP entries use their saved filenames as explicit names, so backup restore preserves document names.
HTML without embedded source remains raw HTML.
Converted HTML never retains a writable link to its original file.

Both URL entry points use `share.receiveUrl()` and the existing incoming-copy decisions.
The source URL remains the stable origin, including when a redirect changes its destination.
A failed query load retains the previously viewed document, as the form does.
All fetching remains in the browser.

`src/io/input.ts` owns input generations and abort signals.
Receiving and navigation check the current operation after asynchronous reads and before applying results.
New files, documents, and URLs supersede previous operations.
Conflict dialogs close when their operation is canceled.
Unchanged Markdown downloads and unchanged merges retain original CRLF and review-block bytes.
Pending storage writes finish before the viewer switches documents.

Initial checks passed 43 unit cases and 63 import browser cases across Chromium, Firefox, and WebKit.
Three additional browser cases reopened actual generated HTML with body-only source in fresh libraries.
The broader run found import-toast and ZIP-name regressions, which are fixed.
The folder-deletion case now expects the heading-derived name for an ordinary Markdown import.
Six focused import/indexing/restore regression cases passed after those fixes.
Responder smoke tests passed 20 cases, and affected Markdown Preview reference cases passed 5 cases.
Final local checks:

- MARS unit tests: 43 passed.
- Full MARS, export, and import browser suites: 238 passed, 2 skipped across Chromium, Firefox, and WebKit.
- The skipped cases require Chromium's software GPU for color-pixel assertions.
- Type checking, build, and deploy-equivalent preview build: passed.
- E2E lint, tools verification, repository tools checks, and `git diff --check`: passed.
- Marketing-site synchronization: skipped because `SECUTILS_TOOLS_PROMO_HOME_INDEX` is not set.

```bash
MARS_BASE_URL=http://localhost:7373 make e2e-tools-test ARGS="mars.spec.ts mars-export.spec.ts mars-import.spec.ts --workers=3 --output=test-results/mars-phase2-final"
```

The phase 1 model-view case now waits for editor initialization before model import.
The view was previously reset by a late startup render between rotation and saving.
This readiness check leaves the deployed application unchanged.

The user requested commit and deployment on 2026-11-07.
Phase 3 handoffs and tool retirement remain outside this change.

### Phase 2 deployment record (2026-11-07)

Implementation commit: `b9f87525 feat(mars): unify imports and simplify export workspace`.

```bash
make deploy-tools ARGS=mars
```

The deployment published MARS, its companion skill, and the four tools-host discovery aggregates.
The application is live at <https://mars.secutils.dev/>.
The deployed MARS bundle matches the local build byte-for-byte.
Its SHA-256 is `6f7fb49dd1d0e4071d1aed3a375db5dd33cafdddaa37a15c36b7527671718614`.
The deployed skill matches the local source after host substitution.
The live response retains COOP `same-origin` and COEP `credentialless`.
Post-deploy browser checks passed 250 cases across Chromium, Firefox, and WebKit, with 2 GPU-specific cases skipped:

```bash
MARS_BASE_URL=https://mars.secutils.dev make e2e-tools-test ARGS="mars.spec.ts mars-export.spec.ts mars-import.spec.ts --workers=3 --output=test-results/mars-post-deploy-phase2"
```

## Phase 3: Regression coverage and useful workflow integration

### Automated coverage

1. Add unit tests for export settings, snapshot isolation, title escaping, and source extraction.
2. Add HTML generation tests for each optional reader feature and source escaping.
3. Add print tests for A4, Letter, Portrait, and Landscape dimensions.
4. Add browser tests for the export workspace and `Ctrl/Cmd+P`.
5. Add browser tests for stale-result prevention, cancellation, Retry, and focus return.
6. Retain Phase 0 native-link tests for custom menus, modifier bypass, and normal-tab navigation.
7. Move reusable rich-rendering cases into shared helpers or MARS-focused tests.
8. Retain fixtures for colored models, ZIP64, invalid geometry, and large models.

Use real output files for offline HTML checks.
Use pixel assertions for colored geometry rather than element presence alone.
Intercept PDF readiness and print requests without opening the system print dialog in automated tests.
Check actual print/save output manually in supported desktop browsers.
Inspect page dimensions, searchable text, pagination, diagram completeness, and model colors.

### Workflow integration

1. Change PDF Extractor's useful preview/export handoff to open MARS.
2. Preserve its Text and Markdown payloads and contextual action availability.
3. Update the handoff's browser test and companion skill.
4. Expand the MARS skill with preview, HTML, PDF, and native-menu instructions.
5. Update the MARS guide, package README, and root feature descriptions.
6. Update tool metadata and registry descriptions if the public feature description changes.
7. Preserve the existing privacy statement about local processing and encrypted links.

These updates describe current features.
They do not require legacy endpoint maintenance, redirects, or a separate retirement phase.
Leave unrelated plans and Markdown Preview files unchanged unless a separately approved cleanup requires them.

### Phase 3 implementation record (2026-11-07)

PDF Extractor's Export menu now provides **Open in MARS**.
Text and Markdown retain their exact payloads and the source PDF document name.
The destination uses `#name=<source-name>&md=<encoded-text>` on the MARS host.
The browser sends neither PDF bytes nor extracted text to a server.
The action remains unavailable for JSON, Outline, Screenshots, and unfinished Markdown.
Both payloads opened actual MARS documents and downloaded unchanged Markdown in Chromium, Firefox, and WebKit.

`e2e/tools/mars-models.spec.ts` adds MARS-focused coverage for binary/ASCII STL, large meshes, ZIP64, and interpolated colors.
The cases also cover component and slicer files, broken archives, invalid geometry, invalid views, and worker cancellation.
Offline checks use actual exported HTML files, and color checks read rendered pixels.
The existing 3MF fixtures remain in place.
The shared importer unit suite passed all 10 cases, including archive and geometry limits.

Export unit tests now switch each optional HTML reader feature independently.
The earlier phases already cover immutable snapshots, title/source escaping, isolated previews, page dimensions, cancellation, Retry, and keyboard focus.
Native-link regression cases remain in the full MARS suite.
The ZIP64 Markdown Preview reference case now waits for the first painted pixels before sampling the canvas.
The original Markdown Preview implementation remains unchanged.

The MARS and PDF Extractor skills describe the handoff, reading, editing, exports, and native browser actions.
The MARS guide, package README, root README, tools index, metadata, and registry describe reading, review, and HTML/PDF export.
The dark/light MARS OG images and own-host JPEG were regenerated and reviewed.
The privacy statements about local processing and encrypted short links remain unchanged.

Final local checks:

- MARS unit tests: 44 passed.
- Shared 3MF importer unit tests: 10 passed.
- Shared-asset unit tests: 6 passed.
- Type checking, build, preview build, E2E lint, tools verification, and repository tools checks: passed.
- Responder smoke cases: 20 passed.
- Affected Markdown Preview reference cases: 9 passed, including ZIP64 after its pixel-readiness correction.
- Full MARS, export, import, model, and PDF Extractor browser suites: 285 passed, 10 skipped across Chromium, Firefox, and WebKit.
- The 10 skips cover GPU-dependent canvas/color assertions outside Chromium. The corresponding Chromium cases passed.
- `git diff --check`: passed.
- Marketing-site synchronization: skipped because `SECUTILS_TOOLS_PROMO_HOME_INDEX` is not set.

```bash
PDF_BASE_URL=http://localhost:7374 MARS_BASE_URL=http://localhost:7373 make e2e-tools-test ARGS="mars.spec.ts mars-export.spec.ts mars-import.spec.ts mars-models.spec.ts pdf.spec.ts --workers=3 --output=test-results/mars-phase3-full"
```

The PDF Extractor preview serves the changed source with its bundled parser at `/pdf` and its skill at `/pdf.md`.
Handoff cases route the MARS destination to the local deploy-equivalent page.
The user requested phase 3 deployment on 2026-11-07.

### Phase 3 deployment record (2026-11-07)

```bash
make deploy-tools ARGS="mars pdf-extractor index"
```

The deployment published MARS, PDF Extractor, both companion skills, the tools index, and four discovery aggregates.
MARS is live at <https://mars.secutils.dev/>.
PDF Extractor is live at <https://tools.secutils.dev/pdf>.
The tools index is live at <https://tools.secutils.dev/>.

The deployed MARS and PDF parser bundles match the local builds byte-for-byte.
The MARS bundle SHA-256 remains `6f7fb49dd1d0e4071d1aed3a375db5dd33cafdddaa37a15c36b7527671718614`.
Both deployed skills match their local sources after host substitution.
Their discovery digests also match.

The deployed MARS OG JPEG matches the regenerated artifact.
Its SHA-256 is `284f40dea0299988a2e063a86ef3aad3f445c2a45659770c6003a72f55b48f74`.
The live MARS response retains COOP `same-origin` and COEP `credentialless`.
The PDF Extractor handoff and updated MARS description are present in the live pages.

Post-deploy checks used the live hosts without local preview overrides:

- MARS, export, import, model, and PDF Extractor suites: 285 passed, 10 GPU-specific cases skipped.
- Tools index and registry suites: 31 passed, 2 failed.
- Complete run: 316 passed, 10 skipped, 2 failed.

Both failures concern the unrelated SAML tool, which this deployment did not publish.
`https://tools.secutils.dev/elastic/saml/idp-login.md` returns 404 instead of a skill.
`https://tools.secutils.dev/elastic/saml/idp-login` returns 404 instead of the expected Markdown-negotiation redirect.
Direct HTTP checks reproduced both 404 responses.
The broader registry verification therefore remains incomplete.
No SAML source, responder, or registry entry changed to bypass these failures.

```bash
env -u MARS_BASE_URL -u PDF_BASE_URL -u BASE_URL make e2e-tools-test ARGS="mars.spec.ts mars-export.spec.ts mars-import.spec.ts mars-models.spec.ts pdf.spec.ts index.spec.ts registry.spec.ts --workers=3 --output=test-results/mars-phase3-post-deploy"
```

### Conditional export-hint removal and redeployment (2026-11-07)

The remaining table-of-contents hint and its display handler are removed entirely.
The Table of contents setting and heading-based output behavior remain unchanged.
A new regression case covers documents without section headings.

`make deploy-tools ARGS=mars` republished MARS, its skill, and the four discovery aggregates.
The live page contains neither the hint text nor its element, and the deployed bundle contains no hint handler.
The bundle matches the local build byte-for-byte.
Its SHA-256 is `dc1cd5a0eac43cb317edd4c205b313ba79f7beb195fdecb5fc9ba10e8f5fa5e9`.

Local checks passed 44 unit cases and 73 export browser cases, with 2 GPU-specific cases skipped.
Type checking, preview build, lint, tools verification, and repository tools checks passed.
Post-deploy MARS, export, import, and model suites passed 266 cases, with 10 GPU-specific cases skipped.
The new hint-removal case passed in Chromium, Firefox, and WebKit on the live host.

### Remaining environment-dependent checks

The browser tools cannot inspect Safari/Edge browser-chrome menus or operate native system print/save dialogs.
The two manual acceptance items therefore remain unchecked.
They are not automated successes.

1. In Safari and Edge, use Shift + right-click on a stored MARS document.
   Inspect the native link menu and the browser's supported split-view commands.
   Opening a normal tab or window must retain the document and leave the source tab unchanged.
2. Open a document with text, Mermaid diagrams, and a colored 3MF model.
   In supported desktop browsers, open its PDF export workspace.
   Save A4/Letter output in both orientations through the system print dialog.
   Inspect searchable text, page dimensions, page numbers, title footers, complete diagrams, and model colors.
3. Set `SECUTILS_TOOLS_PROMO_HOME_INDEX` to the marketing site's `index.html` and run `make tools-check`.
   Marketing-site synchronization remains unverified while this variable is unset.

### Command gate

Run checks for each completed phase, rather than postponing all checks until the final phase.
Use the task guides for any additional checks required by the changed surface.

```bash
npm --prefix dev/tools/js/mars test
npm --prefix dev/tools/js/mars run typecheck
npm --prefix dev/tools/js/mars run build
make tools-verify ARGS="dev/tools/mars.html"
make tools-check
```

Serve the deploy-equivalent MARS page through the package's local serve command.
Run the MARS browser suite against that server.
Include affected shared Markdown cases and PDF Extractor cases.
Run browser-test lint through the existing E2E package commands.

After route, responder, or build-inliner changes, run the applicable standalone smoke tests.
Search-quality evaluation is required only if a phase changes the search implementation or its input contract.
Native-menu and export work does not require model downloads merely to complete its checks.

For any docs output-pipeline change, complete the docs-output guide's gates.
For ordinary feature descriptions, run the checks applicable to those descriptions.
Do not run deployment targets without a deployment request.

## Final acceptance checklist

- [x] Ordinary document right-click still opens the MARS custom menu.
- [x] Shift + right-click leaves document-link context menus uncanceled.
- [x] Native normal-tab/window navigation opens the same stored document in Chromium and Firefox.
- [ ] Manual Safari/Edge native menu contents and browser split-view commands checked.
- [x] The main toolbar gains only one Export control.
- [x] The export workspace separates HTML and PDF settings.
- [x] The actual output previews remain isolated from the MARS library.
- [x] HTML download, Copy HTML, Markdown download, and automated PDF print requests work.
- [ ] Manual system printing and saved PDF output pass the desktop-browser gate.
- [x] Exported HTML retains reader controls and interactive maps/models.
- [x] PDF previews retain readable vector text, diagrams, and colored model snapshots.
- [x] Export preparation leaves the source, threads, and saved views unchanged.
- [x] URL form and query imports use consistent extraction and naming.
- [x] Exported embedded source reopens as editable Markdown.
- [x] PDF Extractor can hand its useful output to MARS.
- [x] All automated phased regression checks pass, and blocked manual checks remain explicit.
- [x] No compatibility or legacy-link migration work enters Phase 0.

## Research references

- MDN, anchor semantics: <https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/a>
- MDN, `contextmenu` and Firefox modifier behavior: <https://developer.mozilla.org/en-US/docs/Web/API/Element/contextmenu_event>
- WAI-ARIA APG, navigation tree with real links: <https://www.w3.org/WAI/ARIA/apg/patterns/treeview/examples/treeview-navigation/>
- Modern Web Guidance, `dialog` and `html` guides, consulted during planning and implementation.

The browser owns menu availability, print destinations, and private-profile storage separation.
Browser prototype gates establish those behaviors without pretending that application DOM tests cover browser chrome.
