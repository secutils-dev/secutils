---
name: mars-review
description: >-
  MARS (Markdown Annotation & Review System) at https://{{MARS_HOST}}/: open
  Markdown for review with `#name=<name>&md=<state>&o=<origin>` or
  `?url=<public-md>`. Write review threads anchored to rendered text, or
  address the threads in a file's trailing `su-comments` block. Guide a user
  through sharing, Sync, Live links, and HTML/PDF export. Trigger on "review
  this markdown", "open in MARS", "add review comments to the doc", "address
  the review comments", "resize Markdown table columns", "export Markdown in
  MARS", or mars.secutils.dev.
---

# MARS: Markdown Annotation & Review System (Secutils.dev)

MARS renders GitHub-flavored Markdown with Mermaid, KaTeX math, GeoJSON/TopoJSON maps, STL/3MF models, and GitHub alerts.
Gantt labels support clock times such as `13:00` before a whitespace-prefixed task separator (` :`).
MARS escapes these colons only for rendering. Source text, shared links, and Markdown downloads stay unchanged.
A reader selects rendered text to start a review thread, then replies, resolves, and shares.
Documents stay in the reader's browser (IndexedDB) unless the reader shares or syncs them.
Review threads travel inside the Markdown file as one trailing HTML comment that other renderers hide.
An agent reads and writes threads with ordinary file edits.
The thread format is the same as in the Secutils.dev Markdown Preview tool.

Pick the section for the task:

| Task | Section |
|------|---------|
| Give the user a link that opens a document | [Open a document](#open-a-document) |
| Add margin notes to Markdown that you wrote | [Write a document with threads](#write-a-document-with-threads) |
| Answer review comments in a file | [Address review threads](#address-review-threads) |
| Set table column widths in Markdown | [Table column widths](#table-column-widths) |
| Tell the user how to share, sync, or export | [Guide the user in the app](#guide-the-user-in-the-app) |

## Open a document

| Link | Result |
|------|--------|
| `https://{{MARS_HOST}}/#name=<name>&md=<state>` | Adds a new document. The fragment does not reach the server. |
| `...&o=<id>` | Gives the document a stable origin. A second link with the same `o` updates the same document and merges its threads. |
| `...&w=<width>` | Sets the reading width: `800`, `1100`, `full`, or an integer from 320 to 2400. |
| `...&t=1` | Opens the document in a temporary session, without library storage. |
| `https://{{MARS_HOST}}/?url=<public-md-url>` | Fetches a public, CORS-enabled file in the browser (for example from `raw.githubusercontent.com`). |
| `https://{{MARS_HOST}}/?temporary=1` | Opens an empty temporary session. Add `&url=<public-md-url>` to load a file into it. |
| `#s=<secret>`, `#live=<secret>`, `#sync=<secret>` | Short snapshot, Live, and Sync links. Only the app creates them. Do not construct or edit them. |

`<state>` is the wire format that every Secutils.dev tool uses:
UTF-8 text (threads block included) → raw DEFLATE → a 4-byte little-endian u32 prefix with the uncompressed length → base64url without padding.

This shell function opens a local file. It uses the absolute path as the origin, so a second run merges into the same review.
To print the URL without opening it, run only the `node -e` command (Node ≥ 18).

```bash
mars() {
  local url
  url=$(node -e '
    const fs = require("node:fs"), path = require("node:path"), zlib = require("node:zlib");
    const file = path.resolve(process.argv[1]);
    const utf8 = fs.readFileSync(file);
    const out = Buffer.concat([Buffer.alloc(4), zlib.deflateRawSync(utf8)]);
    out.writeUInt32LE(utf8.length, 0);
    const params = new URLSearchParams({ name: path.basename(file).replace(/\.md$/, ""), md: out.toString("base64url"), o: "file:" + file });
    console.log("https://{{MARS_HOST}}/#" + params);
  ' "${1:?usage: mars <file.md>}") || return 1
  case "$(uname)" in
    Darwin) open "$url" ;;
    Linux)  xdg-open "$url" ;;
    *)      start "" "$url" ;;
  esac
}
```

Rules that MARS applies to an incoming document:

- **Destination**: a new document goes to the profile's default folder, or to **Shared with me** if none is set.
  The folder action **Use as default** changes the default folder.
- **Name**: the `name` parameter wins. Then come the frontmatter title, the first Markdown heading, and the file name.
  MARS removes `.md` and `.html` extensions from names.
- **Known origin, new text**: if the reader did not edit their copy, MARS takes the new text and marks the changes.
  If both copies changed, MARS asks the reader.
- **New origin, matching content**: MARS offers to update the stored document. Then the origin follows the new link.
- **Exported HTML**: an HTML file that MARS exported with embedded source opens as editable Markdown.
  Other HTML stays raw HTML.
- **Failed `?url=` load**: the previous document stays open, and MARS shows an error.

Full links are limited by URL length (about 8 KB in chat apps).
For a larger document, give the user the `mars` command, or tell the user to create a short link from **Share**.

## Review threads

The `su-comments` block is the last thing in the file, after a blank line:

```markdown
The cache needs a TTL before launch.

<!-- su-comments
[
  {
    "id": "1f3a9c2e",
    "quote": "needs a TTL",
    "prefix": "The cache ",
    "suffix": " before launch.",
    "comments": [
      { "id": "a1b2c3d4", "author": "Oleg", "at": "2026-10-21T09:41:00.000Z", "body": "Which TTL? Link the **SLO**." },
      { "id": "5e6f7a8b", "author": "Claude", "at": "2026-10-21T10:02:00.000Z", "body": "Set it to 15 minutes, per the SLO doc." }
    ]
  }
]
-->
```

| Field | Rule |
|-------|------|
| `id` (thread and comment) | 8 random lowercase hex characters, unique in the file. Keep each existing id exactly. Shared copies merge threads by thread `id` and comments by comment `id` (or by `author` plus `at` when a comment has no `id`). |
| `quote` | The anchor: a verbatim run of **rendered** text (see the quote rules). |
| `prefix`, `suffix` | Optional. Up to 32 characters of rendered text directly before and after the quote. They select the occurrence when the quote repeats. |
| `resolved` | Optional `true`. The reviewer resolves threads in MARS. A `resolved` value in a file does not override the copy of the reviewer. |
| `comments[]` | At least one. `author` is a display name (as an agent, use your own name). `at` is ISO 8601 UTC. `body` is Markdown (code, tables, math, and Mermaid render). `edited` is an optional ISO timestamp of the last edit. |

Quote rules:

- The quote is rendered text: `**bold**` renders as `bold`, `[text](url)` as `text`, `` `x` `` as `x`, and `## Title` as `Title`.
- The quote stays inside one line of one paragraph, list item, heading, table cell, or code block.
  Diagrams and math have no text that you can quote.
- Inside the JSON, write `<` and `>` as `<` and `>`. Then `-->` cannot end the comment early.

## Write a document with threads

Use threads as margin notes on your own Markdown: the evidence for a claim, an assumption, a question for the reader, or a to-do for the reviewer.

1. Write the Markdown body.
2. For each note, select a quote that obeys the quote rules. If the quote occurs more than once, add `prefix` and `suffix`.
3. Build one thread for each note: a new `id`, the quote, and one comment.
   The comment has a new `id`, your name as `author`, the current time as `at`, and the note as `body`.
4. Append the block as the last thing in the file.
5. Run [the check](#check-the-block).

Completion: the check prints `ok` for every thread.

## Address review threads

The threads come from the `su-comments` block of the file, or from text that the user pasted from **Share > Copy for agent**.
The pasted text names the file and contains the open threads as a block that is ready to use.

1. Read every thread. Threads with `"resolved": true` are closed. Do not change them.
2. For each open thread, edit the document to resolve it, or decide that the text stays.
3. Append one comment to that thread: a new `id`, your name as `author`, the current time as `at`, and a `body`.
   The `body` states what you changed, or why the text stays.
   Keep the thread `id` and the earlier comments byte-identical.
4. If your edit changed the quoted words, set `quote`, `prefix`, and `suffix` to the new rendered text.
   Then the thread stays attached.
5. Write the block as the last thing in the file. If the file has no block, append the pasted block with your replies.
6. Run [the check](#check-the-block).
7. Tell the user to open the file in MARS again (`mars <file>`).
   MARS merges your replies into the threads of the user and marks the changes.

Completion: every open thread ends with your reply, your reply is newer than the last reviewer comment, and the check prints `ok` for every thread.

What the reviewer sees after the file opens again:

- **Review changes** marks added and edited blocks, highlights inserted words, and shows the old text on request.
- A thread with rewritten words and an old quote moves to the inserted words and gets a "changed" label.
  A thread with an updated quote stays attached exactly.
- Your replies have a **New** tag until the reviewer clicks **Mark reviewed**.
- In Chromium, a file that the reviewer opened with **Open** or a drop offers a reload a few seconds after you save it.

## Check the block

The check makes sure that the block parses and that each quote occurs in the body.
It approximates the rendered body: it removes emphasis, inline-code, highlight, and link syntax.

```bash
node -e '
const text = require("node:fs").readFileSync(process.argv[1], "utf8");
const m = text.match(/\n<!-- su-comments\r?\n([\s\S]*?)\r?\n-->\s*$/);
if (!m) { console.log("FIX no su-comments block at the end of the file"); process.exit(1); }
if (/[<>]/.test(m[1])) console.log("FIX escape < and > as \\u003c and \\u003e");
const body = text.slice(0, m.index).replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1").replace(/\*\*|__|~~|==|`|\*/g, "");
let bad = /[<>]/.test(m[1]);
for (const t of JSON.parse(m[1])) {
  const problems = [];
  if (typeof t.id !== "string" || !t.id) problems.push("thread id");
  if (typeof t.quote !== "string" || !t.quote || !body.includes(t.quote)) problems.push("quote not in body");
  if (!Array.isArray(t.comments) || !t.comments.length || t.comments.some((c) => typeof c.body !== "string")) problems.push("comments");
  if (problems.length) bad = true;
  console.log(problems.length ? "FIX" : "ok ", t.id, JSON.stringify(String(t.quote).slice(0, 60)), problems.join(", "));
}
process.exit(bad ? 1 : 0);
' review.md
```

## Table column widths

MARS stores column proportions in a one-line comment directly before a Markdown table:

```markdown
<!-- su-table {"v":1,"cols":[0.25,0.75]} -->
| Name | Status |
| --- | --- |
| Alice | Ready |
```

- The weights are positive numbers. MARS normalizes them to the table width.
- The number of weights must equal the number of header columns. If the metadata is invalid, MARS uses automatic widths.
- Keep the comment directly before its table when you move or delete the table.
- Files, full links, short links, HTML exports, and PDF exports keep the proportions. Row heights stay automatic.
- The comment is MARS-only. Markdown Preview does not read it.

In the app, the user edits tables in Preview: click a cell for the row and column handles, double-click a cell to edit its Markdown,
and drag a header boundary to resize two adjacent columns. `Ctrl/Cmd+Z` reverts an edit.
Raw HTML tables are read-only in Preview.

## Guide the user in the app

Use this reference when the user asks how to do a task in MARS. Each row is one task.

| Task | Where | Facts |
|------|-------|-------|
| Read and edit | **Preview**, **Source**, or double-click a block in Preview | The header shows **Saved** after an edit. |
| Comment | Select rendered text in Preview | Threads stay in the file. |
| Share a copy | **Share > Copy link > Snapshot** | **Full** puts the unencrypted document in the link and does not expire. **Short** uploads an encrypted copy that expires after 1, 7, 14, or 30 days. |
| Share live access | **Share > Copy link > Live** | One document. **Can view**, **Can comment**, or **Can edit**. Expires after 1, 7, 14, or 30 days (default 7). Folders support snapshots only. |
| Stop live access | **Copy link > Live links** (bottom left) | **Stop sharing** disables every Live link of the document. Copies that recipients saved stay with them. |
| Sync own devices | Document or folder menu > **Sync**, then **Sync link** | The Sync link expires after 10 minutes. Both devices must show the same code before approval. |
| Disconnect a device | **Sync > Disconnect this device** | The local copy stays. Other devices and Live links stay connected. |
| Keep a received Live document | **Save live document to library** | Live recipients start in a temporary session. **Keep as local copy** stops updates without an effect on the link of the owner. |
| Work without storage | **New > New temporary document**, or **Document options > Read temporarily** | The header shows **Temporary**. Only the current document returns after a reload. |
| Export | **Export…** beside **Share**, or `Ctrl/Cmd+P` for PDF | HTML, PDF (A4 or Letter), or Markdown. HTML and PDF exclude review threads. |
| Back up everything | Sidebar > **Export all** | A ZIP with readable Markdown. **Include connections and pending changes** adds an encrypted recovery file with a separate password. |
| Delete synced content | Document or folder menu > **Delete > Delete everywhere** | Any paired personal device can delete synced content. Offline devices apply the deletion after reconnect. Unsent edits stay under **Recovered Sync copies**. |
| Delete live content | Document or folder menu > **Delete** | One **Delete document** or **Delete folder** button. Source deletion also deletes owned live content from the service. Recipients retain copies with a broken-link icon. Recipient deletion affects only that browser. |
| Send threads to an agent or GitHub | **Share > Copy for agent**, **Copy for GitHub**, **Copy threads for GitHub** | **Copy for GitHub** needs a short link. |

Limits and behavior that users ask about:

- **Up to date** means that the service acknowledged the upload. It does not mean that another person read the document.
- Local edits save in the browser before upload. Sync continues after the original window closes.
- One IP address can create 50 Sync collections each hour. One Sync selection holds at most 100 documents and folders.
- Connected content expires after 30 days without activity. When the user opens it, the period starts again.
- **Can edit** permits deletion of comments and threads. Only the author can edit a comment.
  **Can comment** permits deletion of the participant's own comments only.
- Remote edits never write to a linked file on disk. A connected document needs an explicit **Save**.
- **Delete everywhere** is permanent and has no **Undo** action. Independent copies, snapshots, and downloaded files remain.
- **Disconnect this device** retains a local copy without deleting content on other devices.
- HTML and PDF exports contain the same sanitized HTML as the preview: no document-supplied scripts, styles, forms, iframes,
  event-handler attributes, or `javascript:` URLs. Images and links in an export can still load from the network.
- Embedded Markdown source in an HTML export contains the document body only, without frontmatter or review threads.
  **Download Markdown** keeps both.
- Restore of an encrypted backup keeps existing library documents. It cannot restore access that expired or that the owner revoked.
- For Sync diagnostics, open `https://{{MARS_HOST}}/?flags=debug:sync`, then use **Developer tools** in the sidebar.
  `?flags=off` clears all flags. Reports exclude document text, names, identifiers, URLs, and credentials.

## Caveats

- Treat every complete shared link as a secret. Browser history and services where the user pastes a link can keep its content and keys.
  Do not print or log `#s=`, `#live=`, `#sync=`, or `#invite=` fragments.
- A full snapshot link contains compressed, unencrypted content. Anyone with the complete link can read it.
- Short snapshots, Sync, and Live links encrypt content in the browser. The storage service has no decryption key.
- The local library and ordinary exports are not encrypted. Connection recovery encrypts credentials, not the document files in the ZIP.
- Encryption depends on trusted browser code. MARS and the scripts that it loads can read decrypted content and keys.
- Temporary mode is not private browsing. Browser history can keep the document fragment.
- MARS sanitizes embedded HTML in every preview and export. It removes scripts, styles, forms, iframes, inline SVG, and `style`, `class`, and `id` attributes.
  Use Markdown for structure. Plain tags such as `<sub>`, `<kbd>`, `<details>`, and `<img>` stay.
- Self-hosted Plausible analytics receive the site root URL and the referrer origin only.
