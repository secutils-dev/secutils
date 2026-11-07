---
name: mars-review
description: >-
  MARS (Markdown Annotation & Review System) at https://{{MARS_HOST}}/: open
  Markdown for review with `#name=<name>&md=<state>&o=<origin>` or
  `?url=<public-md>`. Read or edit Markdown, export standalone HTML or PDF,
  and write review threads anchored to its text.
  Reply to or address the review threads in a file's trailing `su-comments`
  block. Trigger on "review this markdown", "open in MARS", "add review
  comments to the doc", "address the review comments", "export Markdown in MARS",
  "preview Markdown in MARS", "resize Markdown table columns", or mars.secutils.dev.
---

# MARS: Markdown Annotation & Review System (Secutils.dev)

MARS renders GitHub-flavored Markdown with Mermaid, KaTeX math, GeoJSON/TopoJSON
maps, STL/3MF models, and GitHub alerts. Readers select text to start a review
thread, then reply, resolve, and share. Documents live in the reader's browser
(IndexedDB); the server sees only encrypted short links. Threads travel inside
the Markdown file as one trailing HTML comment that GitHub and other renderers
hide, so an agent reads and writes them with ordinary file edits. The format is
shared with the Secutils.dev Markdown Preview tool.

## Read and edit

1. Open a Markdown file, a shared document, or a public URL in MARS.
2. Use **Preview** for rendered text, diagrams, math, maps, and models.
3. Use **Source** for Markdown edits, or double-click a text block in Preview.
4. Select rendered text to add a review comment.
5. Use **Export…** for standalone HTML, PDF, or Markdown output.

Completion: the document stays in the browser library, and the header shows **Saved** after an edit.
PDF Extractor's **Export > Open in MARS** opens its Text or Markdown output under the original document name.
The handoff preserves exact text through the URL fragment and uploads no PDF bytes.

## Temporary sessions

1. Use **New temporary document** from the sidebar New menu for a session without library storage.
2. Use **Document options > Read temporarily** for an existing document.
3. Use **Save current to library** only when you want persistent storage.
4. Export other temporary documents before closing the session.

Completion: the header displays **Temporary**, and the banner states that the session is not saved in the library.
Browser storage denial and quota failures select this mode automatically.
Editing, comments, tables, sharing, Markdown downloads, and HTML/PDF output remain available.
Opening a file temporarily does not enable autosave to that file.

The current document stays in the URL fragment and returns after reload, including its name, reading width, and review metadata.
Other session documents and folders require ZIP export. Oversized or unavailable recovery URLs produce a download warning.
Temporary mode is not private browsing. Browser history can retain the document fragment, and browser preferences can still persist.
Temporary short-link deletion tokens stay in the tab and disappear on reload.

Use `?temporary=1` for an empty temporary session.
Append `&t=1` to a full or short fragment to open a shared document temporarily.
Use `?temporary=1&url=<public-md>` for a temporary URL load.

## Table editing

1. In Preview, click or tap a table cell to display its row and column handles.
2. Drag a column boundary in the header to resize the two adjacent columns.
3. Click or tap **Row actions** or **Column actions** to open its action panel.
4. Use the insert and remove buttons for the selected row or column.
5. Drag the row handle vertically or the column handle horizontally to reorder.
6. Use the move buttons or the focused handle's arrow keys for keyboard reordering.
7. Use Left/Right arrow keys on a resize handle to adjust its width.
8. Use **Reset column widths** in Column actions to restore automatic sizing.
9. Use `Ctrl/Cmd+Z` to undo an edit.
10. Press Escape to close an action panel or cancel an unfinished drag.

To edit one cell, double-click it in Preview. With a cell focused, Enter also opens its editor.
The editor contains only that cell's Markdown. The rest of the table stays visible.
Use Escape or `Ctrl/Cmd+Enter` to apply the edit, as with block editing. There are no separate Apply/Cancel buttons.
Use Enter for a line break. MARS stores line breaks as `<br>` and escapes literal pipes.
Clicking outside, sharing, exporting, and switching views apply the current cell edit.

In **Column actions**, choose **Left**, **Center**, or **Right** under **Content alignment**.
The selected alignment applies to the whole column, including its header.
Arrow keys switch the focused alignment control. Cell edits and alignment changes support `Ctrl/Cmd+Z`.

Completion: the header shows **Saved**, and the table layout returns after reopening the file or link.
The header row stays above the body rows, and the table retains at least one column.
Mouse and touch use the same dedicated handles. Table content remains selectable and scrollable.
Handles and action panels follow the visible selection instead of the table's header.
The compact grips align with the selected cell's left and top borders. Hover or focus reveals their action/drag glyph.
Row heights remain automatic.

Column proportions live in a one-line comment directly before the Markdown table:

```markdown
<!-- su-table {"v":1,"cols":[0.25,0.75]} -->
| Name | Status |
| --- | --- |
| Alice | Ready |
```

The positive column weights normalize to the table width.
The weight count must match the header column count.
Invalid metadata falls back to automatic sizing without changing the source.
Row and column actions update the ordinary Markdown cells and alignment markers.
Column moves also move their saved widths.
The comment stays with the table during whole-block moves and deletion.

Files, full links, short links, HTML, and PDF retain the column proportions.
HTML and PDF contain no editing controls.
Embedded Markdown source retains the layout comment for reopening.
Raw HTML tables and tables without reliable source ranges remain read-only in Preview.
This feature is MARS-only. Markdown Preview does not read the layout comment.

## HTML, PDF, and Markdown output

1. Open the document in MARS.
2. Click **Export…** beside **Share**.
3. For standalone reading output, select **HTML**.
4. Use **Download HTML** or **Copy HTML** after the preview is ready.
5. For PDF output, select **PDF** or press `Ctrl/Cmd+P`.
6. Select A4 or Letter, with Portrait or Landscape orientation.
7. Click **Print / Save PDF** after pagination is complete.
8. In the browser print dialog, select Save as PDF or a printer.

MARS does not download PDF bytes or detect whether the reader saved a file.
**Download Markdown** in the export dialog retains frontmatter, line endings, and review threads.
The Share menu keeps review handoffs and links.
Document action menus can export another document without changing the current selection.

The HTML tab lists reading width, table of contents, in-page find, a PDF action, and embedded Markdown source together.
Export width remains separate from the MARS reading width.
The PDF action in exported HTML uses the paper settings from the PDF tab.
Embedded source is readable document-body text, without frontmatter or review threads.
It is not a full backup.

HTML and PDF exclude review threads and review marks.
Both retain complete Mermaid SVGs and saved model views from fence metadata.
HTML maps and models remain interactive without an external renderer.
PDF uses vector text and static color images for WebGL models.
PDF pages always use a light theme, independently of the MARS theme.
Temporary viewer navigation does not change the saved model view.

Paged.js loads only when PDF pagination is needed.
HTML preview and Markdown download do not fetch it.
Images, fonts, math styles, basemap tiles, and PDF pagination can require network access.
An unavailable PDF dependency leaves HTML and Markdown downloads available.

Export settings commit after an output action.
Closing the dialog discards later draft changes.
A document reload shows **Refresh preview**, while the current preview retains its captured version.
The PDF preview centers its pages. Only the dialog footer contains **Print / Save PDF**.
The status appears during preparation or after a failure, and disappears when the preview is ready.

## HTML export restrictions

Exports remove document-supplied `script`, `style`, `iframe`, `object`, `embed`, `base`, `meta`, and `link` elements.
Event-handler attributes and `javascript:` URLs are removed. Internal source-range attributes are removed too.
The exported body excludes frontmatter and review metadata. Markdown downloads retain both.

Ordinary HTML content and inline styling remain. Images, media, links, fonts, and inline CSS can still require network access.
MARS adds its own reader, diagram, map, and model runtimes after this cleanup.
Arbitrary iframe widgets and executable webpage exports are outside this contract.
The cleanup is not a complete HTML sanitizer. Controlled third-party embeds require a separate design and are not necessary for current workflows.

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
| `id` (thread and comment) | 8 random lowercase hex characters, unique in the file. Keep existing ids exactly: shared copies merge threads by thread `id` and comments by comment `id` (or by `author` plus `at` when a comment has none). |
| `quote` | The anchor: a verbatim run of **rendered** text (see the quote rules). |
| `prefix`, `suffix` | Optional. Up to 32 characters of rendered text just before and after the quote. They pick the occurrence when the quote repeats. |
| `resolved` | Optional `true`. The reviewer resolves threads in MARS; a file's `resolved` value does not override the reviewer's copy. |
| `comments[]` | At least one. `author` is a display name (use your own name as an agent), `at` is ISO 8601 UTC, `body` is Markdown (code, tables, math, Mermaid render). `edited` is an optional ISO timestamp of the last edit. |

Quote rules:

- The quote is rendered text: `**bold**` renders as `bold`, `[text](url)` as `text`, `` `x` `` as `x`, and `## Title` as `Title`.
- It sits inside one line of one paragraph, list item, heading, table cell, or code block. Diagrams and math have no quotable text.
- Inside the JSON, `<` and `>` are written as `\u003c` and `\u003e`, so `-->` never ends the comment early.

## Write a document with threads

Use threads to put margin notes on your own Markdown: the evidence behind a claim, an assumption, an open question for the reader, or a to-do for the reviewer.

1. Write the Markdown body.
2. For each note, pick a quote by the quote rules. Add `prefix` and `suffix` when the quote occurs more than once.
3. Build one thread per note: a new `id`, the quote, and one comment with a new `id`, your name as `author`, the current time as `at`, and the note as `body`.
4. Append the block as the last thing in the file.
5. Run the check below.

Completion: the check prints `ok` for every thread.

## Address review threads

The threads come from the file's `su-comments` block, or from text pasted out of MARS with **Share > Copy for agent**. That text names the file and carries the open threads as a ready block.

1. Read every thread. Threads with `"resolved": true` are settled; leave them as they are.
2. For each open thread, edit the document to resolve it, or decide that the text stays.
3. Append one comment to that thread: a new `id`, your name as `author`, the current time as `at`, and a `body` that states what you changed, or why the text stays. Keep the thread `id` and its earlier comments byte-identical.
4. When your edit rewrote the quoted words, set the thread's `quote`, `prefix`, and `suffix` to the new rendered text, so the thread stays attached.
5. Write the block back as the last thing in the file. When the file has no block, append the pasted block with your replies.
6. Run the check below.
7. Tell the user to reopen the file in MARS (`mars <file>`, below). MARS merges your replies into their threads and marks what changed.

Completion: every open thread ends with your reply, newer than its last reviewer comment, and the check prints `ok` for every thread.

## Check the block

Checks that the block parses and that each quote occurs in the body. The body is
approximated by dropping emphasis, inline-code, highlight, and link syntax.

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

## Open in MARS

- **Fragment**: `https://{{MARS_HOST}}/#name=<name>&md=<state>` adds the document to the user's sidebar (under **Shared with me**) and opens it. The fragment never reaches the server.
- **Origin**: add `&o=<id>` for a stable identity. Opening another link with the same `o` merges its threads into the existing copy instead of adding a duplicate. When the text differs, MARS takes the new text and marks the changes if the reader hasn't edited their copy since, and asks otherwise. A link with a new `o` (a moved file) whose threads or text match a stored document offers to update that document, and its origin then follows.
- **Remote URL**: `https://{{MARS_HOST}}/?url=<public-md-url>` fetches a public, CORS-enabled file (for example from `raw.githubusercontent.com`).
- **Short links**: `#s=<secret>` links come from the Share menu only.

Open, file drops, library imports, Load URL, and `?url=` use the same source extraction and naming rules.
An explicit incoming name takes priority over a frontmatter title, a Markdown heading, and the filename fallback.
Filename fallbacks and explicit names lose Markdown and HTML extensions.
Ordinary Markdown imports and unchanged downloads retain CRLF and their original review-block formatting.

An exported HTML file with embedded source opens as editable Markdown.
The source contains only the exported document body, without omitted frontmatter or review threads.
HTML without embedded source remains raw HTML.
MARS does not infer Markdown from rendered HTML or overwrite the original HTML file with Markdown.

The URL form and `?url=` use the same receive path and stable source identity.
Reopening a known source retains the existing incoming-copy decisions.
A new file, document, or URL supersedes a pending load.
Network and CORS failures retain the previous document and display an error.

This shell function opens a local file, using its absolute path as the origin, so
reopening an updated file merges into the same review. Run the `node -e` command
alone to print the URL instead of opening it (Node ≥ 18).

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

Wire format of `<state>`, shared by every Secutils.dev tool: the UTF-8 text
(threads block included) → raw DEFLATE → prefixed with its uncompressed length
as a 4-byte little-endian u32 → base64url without padding.

## Shared reading layout

Full and short links from Share retain the document reading width as well as column proportions.
The received document retains this width after reload, without changing the reader's saved browser preference.
A narrower viewport can still wrap text because table columns remain proportional.

Full links carry an optional `w` fragment parameter: `800`, `1100`, `full`, or a custom integer from 320 to 2400.
Encrypted bundle documents carry the same value in an optional `readingWidth` field.
Invalid or absent values retain the reader's preference.
An explicit `w` fragment parameter also supplies a reading width for older short links.
For example, `#s=<secret>&w=1100` opens an older shared document at the wide reading width.

## What the reviewer sees

- Right-click a document in the sidebar for MARS actions. **Shift + right-click** its name or icon for browser actions.
- Ctrl/Cmd + click, middle-click, and Shift + click retain native browser navigation, including in document search results.
- Sidebar links open the stored document in the same browser profile. Use Share for another profile or a private window.
- The `⋮` button always opens MARS actions. Native split-view commands depend on the browser.
- Document links in Preview open in a new tab. Fragment links still scroll within the document.
- Mermaid diagrams support zoom and pan in the preview and fullscreen. **Fit** restores the complete diagram.
- Readers can drag, scroll, or swipe to pan. Ctrl/⌘ + scroll and trackpad pinch zoom at the pointer.
- The focused diagram supports arrow keys for pan, `+` and `-` for zoom, and `0` for fit.
- The percentage button shows the diagram at actual size. View changes do not alter the Markdown or shared links.
- Unchanged Mermaid diagrams retain their views after unrelated block or Source edits. Changed diagrams start in Fit mode.
- Diagram views last until the reader opens a document or reloads the page. Shared links do not store these views.
- **Pause indexing** survives reloads and pauses all MARS tabs in the same browser. **Resume indexing** resumes those tabs. Search still uses the stored index while paused.
- Selection formatting works across wrapped lines within one list paragraph and preserves source indentation.
- **Bold**, **Italic**, **Strikethrough**, **Highlight**, and **Inline code** work in GitHub alert bodies, including nested blockquotes.
  Formatting preserves alert directives and quote prefixes. Generated alert titles offer **Comment** only.
- Block controls stay in a separate left gutter at every reading width. Hovering over left-side block whitespace also shows them.
- The reading width in **Options** persists across reloads and new tabs, including full and custom widths.
- Between-item gaps keep the adjacent item's controls active, including in nested lists.
- List items can move past multiline siblings. Ordered-list moves renumber the source and preserve the starting number, marker style, and nested content.
- **Mark done** applies only to headings and existing task checkboxes. Regular list items support manual strikethrough through the selection toolbar.
- When a document they reviewed comes back with new text, **Review changes** marks added and edited blocks, highlights inserted words, and shows the old text on request.
- A thread whose quoted words you rewrote, without re-quoting it, follows them to the inserted words of the edited block and is labeled as changed. A thread you re-quoted stays attached exactly.
- Your replies carry a **New** tag until they click **Mark reviewed**.
- In Chromium, a file linked to MARS (opened with **Open** or dropped on the preview) offers a reload within seconds of your save, or reloads by itself when the reader turned that on.

## Caveats

- Full links are bounded by URL limits (about 8 KB in chat apps). For a larger document, hand over the `mars` command, or tell the user to share a short link from the Share menu.
- MARS renders embedded HTML. Keep `<script>` and other untrusted HTML out of documents you hand over.
