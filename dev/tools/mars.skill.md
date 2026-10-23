---
name: mars-review
description: >-
  Open Markdown for review in MARS (Markdown Annotation & Review System) on
  Secutils.dev: threaded review comments anchored to highlighted text, a
  sidebar of reviews and folders kept in the browser, and share links that land
  in a colleague's own sidebar. Renders GitHub-flavored Markdown, Mermaid,
  GeoJSON/TopoJSON maps, STL/3MF models, KaTeX math, and GitHub alerts. Hand
  the user https://{{MARS_HOST}}/ with the Markdown encoded in the URL
  fragment, or `?url=<public-md>`. Trigger when the user asks to "review this
  markdown", "open this in MARS", "open the review file", "share a review",
  "annotate this doc", or names mars.secutils.dev.
---

# MARS: Markdown Annotation & Review System (Secutils.dev)

MARS is a browser app for reviewing Markdown. The rendered preview is the main
view. Readers select text to start a comment thread, reply, resolve, and share
the document. Every document is kept in the browser (IndexedDB) and listed in
the hideable **Documents** sidebar with folders.

## What the user can do

- **Review** - select text in the preview, then click the speech bubble (or
  `Ctrl/Cmd+Alt+M`) to start a thread. Threads sit in the right margin; reply,
  edit, resolve, or delete them. Comments are Markdown and render like the
  document (code, tables, math, Mermaid).
- **Edit** - edit blocks directly in the preview (double-click a block), format
  a selection (bold, italic, strikethrough, `==highlight==`, code), or switch to
  **Source** (`Ctrl/Cmd+E`). Edits save to the browser automatically.
- **Organize** - the **Documents** sidebar holds folders and documents. Drag rows to move or
  reorder them, double-click or press F2 to rename, and use the row menu to
  duplicate, delete (with undo), export, or share. Drop `.md` files, folders, or
  a `.zip` export onto the sidebar to import them.
- **Share** - the **Share** menu copies a short link (encrypted, 1/7/14 days),
  a full link (the review in the URL), a titled link for Slack, or a GitHub
  comment that lists the open threads with their quotes. Folder menus share a
  whole folder. A recipient's copy opens in their own sidebar under
  **Shared with me**. When they share it back, MARS merges their comments into
  the original review.
- **Export** - download one document as `.md`, a folder or everything as
  `.zip`, or (Chromium) write a folder to disk.
- **Linked files** - in Chromium, a file opened with **Open** or dropped on the
  preview stays linked: edits save back to it, and a file changed on disk
  reloads when the browser copy has no unsaved edits.

## Review comments in the file

Threads are stored in the Markdown itself, as one HTML comment at the end of
the file that GitHub and other renderers hide. The format is shared with the
Secutils.dev Markdown Preview tool:

```markdown
<!-- su-comments
[
  {
    "id": "1f3a9c2e",
    "quote": "warnings as the final behavior",
    "prefix": "I disagree with ",
    "suffix": " for a protected query.",
    "comments": [
      { "id": "a1b2c3d4", "author": "Oleg", "at": "2026-10-21T09:41:00.000Z", "body": "Too **strong**?" }
    ]
  }
]
-->
```

A thread attaches to the rendered text that equals `quote`; `prefix` and
`suffix` (up to 32 characters) pick the occurrence when it repeats. Optional
fields: `resolved: true` on a thread, `edited` (ISO timestamp) on a comment.
Comment `id`s let shared copies merge; comments without one are matched by
`author` and `at`. `<` and `>` inside the JSON are written as `\u003c` and
`\u003e`. To pre-seed review threads, append this block to the Markdown before
encoding it.

## Open paths

1. **Fragment** - `https://{{MARS_HOST}}/#name=<name>&md=<state>` adds the
   document to the user's sidebar (under **Shared with me**) and opens it. The
   plain `#<state>` form also works. The fragment never reaches the server.
2. **Origin** - add `&o=<id>` to give the document a stable identity. Opening
   another link with the same `o` merges its comments into the existing copy
   instead of adding a duplicate, and offers to keep or replace the text when
   it differs.
3. **Remote URL** - `https://{{MARS_HOST}}/?url=<public-md-url>` fetches a
   public, CORS-enabled file (for example `raw.githubusercontent.com`).
4. **Short links** - `#s=<secret>` links come from the Share menu only.

## Wire format (URL state)

Same canonical format as every Secutils.dev tool:

```
| 4 bytes uncompressed-length (LE u32) | N bytes raw DEFLATE of UTF-8 markdown |
```

UTF-8 bytes → `deflate-raw` → prepend the 4-byte LE length of the
**uncompressed** text → base64url (`+`→`-`, `/`→`_`, strip `=`). The state is
the Markdown text itself, including any `su-comments` block.

## Directing the user

From any machine with Node ≥ 18:

```bash
node -e '
const zlib = require("node:zlib");
const md = process.argv[1];
const utf8 = Buffer.from(md, "utf8");
const out = Buffer.concat([Buffer.alloc(4), zlib.deflateRawSync(utf8)]);
out.writeUInt32LE(utf8.length, 0);
console.log("https://{{MARS_HOST}}/#" + new URLSearchParams({ name: "Review", md: out.toString("base64url") }));
' '# Hello

Please review **this**.'
```

## Open a local review file from the terminal

The browser cannot read local files directly, so encode the file into the
fragment and open the URL. Add this `mars` function to `~/.zshrc` or
`~/.bashrc` (Node ≥ 18; `open` on macOS, `xdg-open` on Linux). The file path is
used as the origin, so reopening an updated file merges into the same review:

```bash
mars() {
  local file="${1:?usage: mars <file.md>}"
  local url
  url=$(node -e '
    const fs = require("node:fs"), path = require("node:path"), zlib = require("node:zlib");
    const file = path.resolve(process.argv[1]);
    const utf8 = fs.readFileSync(file);
    const out = Buffer.concat([Buffer.alloc(4), zlib.deflateRawSync(utf8)]);
    out.writeUInt32LE(utf8.length, 0);
    const params = new URLSearchParams({ name: path.basename(file).replace(/\.md$/, ""), md: out.toString("base64url"), o: "file:" + file });
    console.log("https://{{MARS_HOST}}/#" + params);
  ' "$file") || return 1
  case "$(uname)" in
    Darwin) open "$url" ;;
    Linux)  xdg-open "$url" ;;
    *)      start "" "$url" ;;
  esac
}
```

Then `mars review.replies.md` opens the file as a review. In Chromium, the
user can also drag the file onto the preview to keep it linked to disk.

## After producing

Hand over the URL. The user reviews, comments, and shares from the browser.

## Caveats

- Reviews live in the user's browser storage. Clearing site data deletes them;
  **Export all** makes a `.zip` backup.
- Full links are bounded by URL limits (about 8 KB in chat apps). Short links
  hold up to 1 MiB after compression; the server stores only ciphertext, and
  link creation is rate-limited per IP (20 links or 16 MiB per hour).
- Embedded HTML in the Markdown is rendered. Don't share untrusted Markdown
  that contains `<script>`.
- Mermaid, KaTeX, topojson-client, and the CodeMirror editor lazy-load from
  CDNs and degrade gracefully when unreachable.
