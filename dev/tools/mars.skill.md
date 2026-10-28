---
name: mars-review
description: >-
  MARS (Markdown Annotation & Review System) at https://{{MARS_HOST}}/: open
  Markdown for review with `#name=<name>&md=<state>&o=<origin>` or
  `?url=<public-md>`; write Markdown with review threads anchored to its text;
  reply to or address the review threads in a file's trailing `su-comments`
  block. Trigger on "review this markdown", "open in MARS", "add review
  comments to the doc", "address the review comments", or mars.secutils.dev.
---

# MARS: Markdown Annotation & Review System (Secutils.dev)

MARS renders GitHub-flavored Markdown with Mermaid, KaTeX math, GeoJSON/TopoJSON
maps, STL/3MF models, and GitHub alerts. Readers select text to start a review
thread, then reply, resolve, and share. Documents live in the reader's browser
(IndexedDB); the server sees only encrypted short links. Threads travel inside
the Markdown file as one trailing HTML comment that GitHub and other renderers
hide, so an agent reads and writes them with ordinary file edits. The format is
shared with the Secutils.dev Markdown Preview tool.

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

## What the reviewer sees

- **Pause indexing** survives reloads and pauses all MARS tabs in the same browser. **Resume indexing** resumes those tabs. Search still uses the stored index while paused.
- Selection formatting works across wrapped lines within one list paragraph and preserves source indentation.
- **Mark done** applies only to headings and existing task checkboxes. Regular list items support manual strikethrough through the selection toolbar.
- When a document they reviewed comes back with new text, **Review changes** marks added and edited blocks, highlights inserted words, and shows the old text on request.
- A thread whose quoted words you rewrote, without re-quoting it, follows them to the inserted words of the edited block and is labeled as changed. A thread you re-quoted stays attached exactly.
- Your replies carry a **New** tag until they click **Mark reviewed**.
- In Chromium, a file linked to MARS (opened with **Open** or dropped on the preview) offers a reload within seconds of your save, or reloads by itself when the reader turned that on.

## Caveats

- Full links are bounded by URL limits (about 8 KB in chat apps). For a larger document, hand over the `mars` command, or tell the user to share a short link from the Share menu.
- MARS renders embedded HTML. Keep `<script>` and other untrusted HTML out of documents you hand over.
