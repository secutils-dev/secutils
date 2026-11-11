# MARS

MARS (Markdown Annotation & Review System) is a browser app to read, edit, and review Markdown.
It runs at <https://mars.secutils.dev/>. It needs no account, no extension, and no local helper.

- Agents: read [SKILL.md](SKILL.md) (served at `/llms.txt`) to use MARS, and [AGENTS.md](AGENTS.md) to change it.
- Sync internals: [src/sync/README.md](src/sync/README.md). Search internals: [src/search/README.md](src/search/README.md).

## What MARS does

- Renders GitHub-flavored Markdown with Mermaid diagrams, KaTeX math, GeoJSON/TopoJSON maps, STL/3MF models, and GitHub alerts.
- Keeps documents and folders in the browser. A document survives a reload without a server.
- Lets a reader select text and start a review thread. Threads travel inside the Markdown file.
- Shares a document as a snapshot link or as a Live link with view, comment, or edit access.
- Syncs selected documents and folders between your own devices.
- Exports standalone HTML, PDF, and Markdown.
- Shows what changed when a reviewed document comes back with new text.

## Privacy and analytics

The Privacy dialog in the app is the full policy. This table is the summary.

| Mode | Where the content goes | Encrypted by MARS | Expiry |
|------|------------------------|-------------------|--------|
| Local library | IndexedDB in this browser | No | Until you delete it or clear site data |
| Download, ZIP export | A file on your disk | No | None |
| Full snapshot link | The URL fragment only. MARS does not upload it. | No (compressed only) | None |
| Short snapshot link | Encrypted copy on the MARS service | Yes, in the browser | 1, 7, 14, or 30 days |
| Sync | Encrypted copy on the MARS service | Yes, in the browser | 30 days without activity |
| Live link | Encrypted copy on the MARS service | Yes, in the browser | 1, 7, 14, or 30 days |

Three facts apply to every mode:

- **A shared link is a secret.** The link contains the key or the content. Browser history and each service where you paste the link can keep it.
- **The service cannot decrypt.** It stores ciphertext, signed headers, and access records. It has no master key and no recovery key.
- **Encryption needs trusted browser code.** MARS and the scripts that it loads can read decrypted content and keys.

MARS bundles the official Plausible tracker and sends events to a self-hosted Plausible instance.
Before an event leaves the browser, `src/platform/analytics.ts` rebuilds it from an allowlist:
the site root URL, the domain, the referrer origin, the event name, and the tracker version.
Document names, content, URL queries, fragments, and link secrets never enter an event.
Link-click, file-download, form, and hash-route tracking are off. The tracker has no `window.plausible` binding.

## Use MARS

### Documents and folders

- **Open** a file, drop it on the page, paste Markdown, or use **Load URL**. The browser fetches a URL directly, without a proxy.
- New files, links, and URLs go to the default folder. The folder action **Use as default** changes it. A star marks the default folder.
- Right-click a document for MARS actions. **Shift + right-click** shows the browser link menu. The `⋮` button always opens MARS actions.
- Each document has a link of the form `#doc=<id>`. The link works only in the same browser profile.
- Every deletion asks for confirmation. **Undo** restores a deleted document as a local copy.

An HTML file that MARS exported with embedded source opens as editable Markdown. Other HTML stays raw HTML.

### Review

- Select rendered text in Preview to add a comment. Reply, edit, resolve, or delete in the thread card.
- Double-click a block in Preview to edit only that block. **Source** opens the full Markdown editor.
- The selection toolbar applies bold, italic, strikethrough, highlight, and inline code. `Ctrl/Cmd+Z` reverts an edit.
- When a document comes back with new text, **Review changes** marks added and edited blocks until you click **Mark reviewed**.
- **Share > Copy for agent** gives an AI agent the file name, the open threads, and the reply steps.

### Share

**Share > Copy link** opens one dialog for all links.

| Link | Use it to | Notes |
|------|-----------|-------|
| Snapshot, Full | Send a copy that needs no server | The link contains the document. Long documents make long links. |
| Snapshot, Short | Send a copy as a short link | The browser uploads an encrypted copy. You can delete the link early from the same browser. |
| Live | Let another person view, comment, or edit one document | Changes arrive without a new link. You can disable the link at any time. |

Folders support snapshot links only. A link also carries the reading width and the table column widths.

### Sync your devices

1. Open the document or folder menu and select **Sync**.
2. Select **Sync link** and open that link on the other device. The link expires after 10 minutes.
3. Make sure that both devices show the same code. Then approve the code on the first device.
4. On the other device, keep the download dialog open until **Sync connected** appears.

After setup, the first window can close. New documents in a synced folder join Sync automatically.
**Disconnect this device** keeps the local copy and leaves other devices connected.

Limits: one IP address can create 50 Sync collections each hour, and one selection holds at most 100 documents and folders.
Connected content expires after 30 days without activity. When you open a document or folder, the period starts again.

### Live links

- The owner selects **Can view**, **Can comment**, or **Can edit**, and an expiry of 1, 7, 14, or 30 days.
- A recipient opens the document in a temporary session. **Save live document to library** keeps it.
- **Keep as local copy** stops updates for that recipient only. **Stop sharing** disables every Live link of the document.
- A recipient can forward an active link with **Copy link > Live**. The access and the expiry do not change.
- Revocation stops future access. It cannot erase a copy that a recipient saved.

The header shows the connection status. **Up to date** means that the service acknowledged your upload.
It does not mean that another person read the document.

### Backup and recovery

**Download** and **Export all** write readable Markdown, or a ZIP of it.
If the selection contains connected documents, the option **Include connections and pending changes** adds one encrypted recovery file.
That file holds the Sync and Live credentials and the changes that are not uploaded. It uses a separate password of 12 or more characters.

To restore, import the ZIP and select **Restore connections**. Existing library documents do not change.
Close MARS on the old device before you restore its credentials in another profile.

CAUTION: Keep the backup password. Secutils.dev cannot recover it.

### Delete

| Choice | Result |
|--------|--------|
| **Delete local copy** | Removes the document from this browser. Other devices and shared links stay active. |
| **Delete from collection** (owner only) | Ends Sync and Live access for everyone. Copies that other devices saved stay there. |

Short snapshot links stay available until they expire. To delete one early, use **Share** before you delete the document.

### Temporary sessions

A temporary session keeps documents in memory and does not read or write the library.

- Start one with **New > New temporary document** or **Document options > Read temporarily**.
- MARS also starts one when the browser denies storage or the storage quota is full.
- The address bar holds the current document, so a reload restores it. Export other documents before you close the tab.
- **Save current to library** moves the current document into the library.

Temporary mode is not private browsing. Browser history can keep the document fragment.

### Export workspace

**Export…** opens a dialog with HTML and PDF tabs. `Ctrl/Cmd+P` opens the PDF tab.

- **HTML**: a standalone file with optional table of contents, find, PDF button, and embedded Markdown source.
- **PDF**: A4 or Letter, portrait or landscape. **Print / Save PDF** sends the prepared pages to the browser print dialog.
- **Download Markdown** keeps frontmatter, line endings, and review threads. HTML and PDF exclude review threads.

An export contains the same sanitized HTML as the preview (see [Embedded HTML](#embedded-html)).
Images, fonts, and map tiles in an export can still load from the network.

### Embedded HTML

A document can come from another person, and the page holds your library and your connection keys.
For that reason MARS sanitizes the HTML in every document, comment, and export before it shows it.

- Removed: `script`, `style`, `iframe`, `object`, `embed`, `form` controls, inline `svg` and `math`, event handlers, and `javascript:` URLs.
- Removed attributes: `style`, `class`, `id`, `name`, and `data-*`.
- Kept: text-level and structural tags such as `<sub>`, `<kbd>`, `<br>`, `<details>`, `<img>`, and `<a href>`.

The Markdown source does not change. **Download Markdown** keeps the original HTML.

### Table editing

Click a table cell in Preview to show its row and column handles.

- Double-click a cell to edit its Markdown. Escape or `Ctrl/Cmd+Enter` applies the edit. Enter adds a line break (`<br>`).
- Drag a header boundary to resize two adjacent columns. Drag a handle to move a row or a column.
- The handle menus insert, remove, move, and align. Each action is one undo step.

MARS stores column proportions in a one-line comment before the table. Files, links, and exports keep it:

```markdown
<!-- su-table {"v":1,"cols":[0.25,0.75]} -->
| Name | Status |
| --- | --- |
| Alice | Ready |
```

Raw HTML tables are read-only. Markdown Preview does not read the comment.

### Search

The sidebar search box matches document names, folder names, document text, and review comments.
Smart search is optional and off by default. It finds passages by meaning with an embedding model that runs in the browser.
The model downloads from Hugging Face once. Document text and queries stay in the browser.
See [src/search/README.md](src/search/README.md) for models, devices, and index behavior.

### Linked files

In Chromium, a file that you open with **Open** or a drop stays linked to the document.
MARS offers a reload when the file changes on disk. **Reload linked files automatically** makes the reload automatic.
Firefox and Safari do not support the necessary file API. Remote edits from Sync or Live never write to a linked file.

## Develop

Run commands in `dev/tools/mars/`.

```bash
npm ci
npm test            # Unit tests (node --test)
npm run typecheck
npm run format      # Prettier. CI runs `npm run format:check`.
npm run serve       # Build, then serve http://localhost:7373/ and /llms.txt
```

`npm run serve` builds the same page that deployment serves. It runs the responder script on an in-memory key-value store,
so short links, Sync, and Live links work offline.
[AGENTS.md](AGENTS.md#verify-a-change) lists the browser tests and the full verification procedure.

### Layout

| Path | Contents |
|------|----------|
| `index.html` | The page shell, the Privacy and Credits dialogs, and the snapshot responder script |
| `src/main.ts`, `src/app.ts` | Startup and the document tree state |
| `src/doc/` | Renderer, HTML sanitizer, block and table editing, review changes, diagrams |
| `src/share/`, `src/io/` | Links, incoming copies, imports, and downloads |
| `src/store/`, `src/tree/` | IndexedDB or memory storage, and the tree model |
| `src/sync/` | Sync and Live links: client engine, cryptography, and the responder. `manager/` holds the connection manager. |
| `src/search/`, `eval/` | Sidebar search, smart search, and the search-quality evaluation |
| `src/export/` | HTML and PDF export |
| `src/platform/`, `src/ui/` | Analytics, feature flags, diagnostics, and UI primitives |
| `test/` | Unit tests. Browser tests are in `e2e/tools/mars*.spec.ts`. |

`npm run build` writes `dist/mars.js` (the app) and `dist/mars-sync-responder.js` (the Sync part of the responder).
Deployment inlines the app into `index.html` and publishes it as a Secutils.dev webhook responder.
`package.json` pins every dependency to an exact version.

### Developer flags and diagnostics

Flags belong to the browser profile. A change updates open tabs and survives a reload.

```text
?flags=debug,experimental              # Replace all flags
?flags=debug:sync,experimental:live_links  # Scoped flags
?flags=debug,-debug:sync               # Category flag with a scoped opt-out
?flags=off                             # Clear all flags
?debug=sync&experimental=live_links    # Change one category only
?debug=off                             # Clear one category
```

While a flag is set, **Developer tools** appears beside Privacy and Credits.
It holds **Copy debug report** and the controls that turn flags off.
The console API does the same without a dialog:

```js
marsFlags.set('debug:sync,experimental:live_links');
marsFlags.enable('debug', 'sync');
marsFlags.disable('debug');
marsFlags.clear();
marsFlags.list();
copy(marsDebug.report());
```

A report contains browser capabilities, timing, counts, setup steps, HTTP statuses, and fixed error codes.
It excludes document names, content, identifiers, URLs, keys, and raw errors.
The latest 500 trace events stay in tab memory. MARS never uploads a report.
Experimental flags are client opt-ins, not authorization.

### Isolated browser-test data

Browser tests that write to the service use `e2e/tools/_mars_test.ts`, not the plain Playwright `test`.
The fixture registers a test scope, and every record of the test stays under `testing/v1/data/<scope>/`.
User records are outside this namespace. Teardown deletes the complete scope.

Only the Node test process holds the operator credential. The browser gets a scope capability that expires after one hour.
The local preview and the tests use the operator key `local-operator-secret` by default.
To use another key, set `MARS_STATS_API_KEY` for the preview and `MARS_TEST_OPERATOR_KEY` for the tests to the same value.

If a run stops before teardown, run the cleanup from the repository root:

```bash
# List test scopes. The output contains no capabilities and no user content.
node --env-file=.env scripts/mars-test-cleanup.ts --list https://mars.secutils.dev
# Remove expired or interrupted test scopes only.
node --env-file=.env scripts/mars-test-cleanup.ts --expired https://mars.secutils.dev
# Resume one recorded scope after a lost cleanup response.
node --env-file=.env scripts/mars-test-cleanup.ts <private-manifest.json> https://mars.secutils.dev
```

CAUTION: Do not clear `sync/v1/` or the snapshot prefixes as test cleanup. Those prefixes hold user data.
