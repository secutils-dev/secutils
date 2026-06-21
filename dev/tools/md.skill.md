---
name: markdown-preview
description: >-
  Render and read Markdown in the browser with the Secutils.dev Markdown
  Preview tool. Reading-first: the rendered preview is the default full-width
  view, with on-demand source editing and self-contained HTML/PDF export.
  Supports GitHub-flavored Markdown, Mermaid diagrams, KaTeX math, GitHub
  alerts (`[!NOTE]` etc.), `==highlights==`, heading anchors, and
  find-in-page. Hand the user https://tools.secutils.dev/md (optionally with
  the Markdown encoded in the URL fragment for one-click preload, or a
  `?url=<public-md>` to fetch a remote file). Trigger when the user asks to
  "preview this markdown", "open this README in a reader", "render markdown
  to read it", "share a markdown preview link", "open a markdown file from
  the terminal in the browser", or anything that names secutils.dev/md.
---

# Markdown Preview (Secutils.dev)

A reading-first, in-browser Markdown reader. The rendered preview is the
default central view; source editing and export are available on demand. It
shares the rendering and export pipeline of the
[Markdown to HTML](https://tools.secutils.dev/md-to-html) tool, with a
richer reader feature set.

## What it renders

- **GitHub-Flavored Markdown** - tables, task lists, ~~strikethrough~~,
  autolinks.
- **Syntax highlighting** - fenced code blocks via highlight.js, with a
  language label and copy button in exports.
- **Mermaid diagrams** - fenced ```` ```mermaid ```` blocks render to inline
  SVG (pre-rendered, so identical in preview, HTML export, and PDF).
- **KaTeX math** - inline `$E = mc^2$` and display `$$ … $$` / `\[ … \]`.
  KaTeX is lazy-loaded from a CDN the first time math is detected.
- **GitHub alerts** - `[!NOTE]`, `[!TIP]`, `[!IMPORTANT]`, `[!WARNING]`,
  `[!CAUTION]` blockquotes become titled callout boxes.
- **`==highlight==`** - rendered as `<mark>`.
- **Heading anchors** - `h1`–`h6` get slug `id`s and a hover anchor link
  (click copies a deep link).
- **Wide tables** - tables with ≥ 4 columns get a horizontal-scroll wrapper.

YAML frontmatter (`title: …`) is honoured: the `title` becomes the document
name used for the page title and downloaded filenames.

## How to use it

- **View switch** - a segmented control toggles between three views:
  **Preview** (rendered, default), **HTML** (the exact self-contained export
  rendered in a sandboxed iframe), and **Source** (the editor). The active
  segment is highlighted; `Esc` returns to Preview.
- **Read** - the rendered preview fills the page. Use **Find** (the search
  icon, or `Ctrl/Cmd+F`) to search within the document; Enter / Shift+Enter
  cycle matches.
- **Edit** - click **Source** (or `Ctrl/Cmd+E`) to swap to a CodeMirror
  pane; edits update the preview live. `Ctrl/Cmd+S` downloads the `.md`;
  `Esc` returns to the preview.
- **Open** - the **Open** button (or `Ctrl/Cmd+O`), drag-and-drop a file
  onto the page, or just paste (`Ctrl/Cmd+V`) into the empty preview.
- **Share** - copies a `tools.secutils.dev/md#<encoded>` URL with the whole
  Markdown round-tripped through the fragment.
- **Export** - **Download HTML** (single self-contained file), **Download
  PDF** (`Ctrl/Cmd+P`, paginated via Paged.js), or **Copy HTML**.
- **HTML options** (gear) - toggles that affect the self-contained HTML
  export: table of contents, PDF export button, in-page find, and embedding
  the Markdown source.

## Open paths

1. **Fragment** - `https://tools.secutils.dev/md#<encoded>` preloads the
   encoded Markdown (see Wire format). This stays entirely client-side.
2. **Remote URL** - `https://tools.secutils.dev/md?url=<public-md-url>`
   fetches and renders a public Markdown file. The fetch happens in the
   user's browser; the URL must be CORS-enabled (e.g.
   `https://raw.githubusercontent.com/…/README.md`). If the URL (or an
   opened/dropped file) is an **HTML document exported by this tool with the
   "Embed source" option**, the original Markdown is pulled back out of its
   `<script type="text/markdown">` block instead of being rendered as HTML -
   so an exported `.html` round-trips losslessly back to editable source.

## Wire format (URL state)

Same canonical format every Secutils.dev tool uses:

```
| 4 bytes uncompressed-length (LE u32) | N bytes raw DEFLATE of UTF-8 markdown |
```

Pipeline: UTF-8 bytes of the raw Markdown → `deflate-raw` → prepend the
4-byte LE u32 of the **uncompressed** length → base64url (`+`→`-`, `/`→`_`,
strip `=`). The state is just the Markdown text itself - no JSON wrapper.

## Directing the user

Plain tool URL for an empty start:

```
https://tools.secutils.dev/md
```

If you already have the Markdown and want them to land on a pre-filled
reader (no copy-paste), encode it into the fragment. From any machine with
Node ≥ 18:

```bash
node -e '
const zlib = require("node:zlib");
const md = process.argv[1];
const utf8 = Buffer.from(md, "utf8");
const out = Buffer.concat([Buffer.alloc(4), zlib.deflateRawSync(utf8)]);
out.writeUInt32LE(utf8.length, 0);
const enc = out.toString("base64").replace(/\+/g,"-").replace(/\//g,"_").replace(/=+$/,"");
console.log("https://tools.secutils.dev/md#" + enc);
' '# Hello

This is **markdown**.'
```

## Open a local file from the terminal ("md-preview README.md")

The browser cannot read local files directly, so the trick is to encode the
file **client-side** into the fragment and hand the browser the resulting
URL. Drop this `mdpreview` shell function into your `~/.zshrc` / `~/.bashrc`
(needs Node ≥ 18; `open` on macOS, `xdg-open` on Linux, `start` on Windows):

```bash
mdpreview() {
  local file="${1:?usage: mdpreview <file.md>}"
  local url
  url=$(node -e '
    const fs = require("node:fs"), zlib = require("node:zlib");
    const utf8 = fs.readFileSync(process.argv[1]);
    const out = Buffer.concat([Buffer.alloc(4), zlib.deflateRawSync(utf8)]);
    out.writeUInt32LE(utf8.length, 0);
    const enc = out.toString("base64").replace(/\+/g,"-").replace(/\//g,"_").replace(/=+$/,"");
    console.log("https://tools.secutils.dev/md#" + enc);
  ' "$file") || return 1
  case "$(uname)" in
    Darwin) open "$url" ;;
    Linux)  xdg-open "$url" ;;
    *)      start "" "$url" ;;
  esac
}
```

Then `mdpreview README.md` opens the rendered file in your browser. The file
never leaves your machine: the fragment (everything after `#`) is **never**
sent to the Secutils.dev server.

For a public file already on the web, skip the encoding and just open the
`?url=` form:

```bash
open "https://tools.secutils.dev/md?url=https://raw.githubusercontent.com/owner/repo/main/README.md"
```

## After producing

If you've handed over the URL, that's the whole interaction - the user takes
it from there in the browser. No follow-up encoding required.

## Caveats

- The Markdown only ever exists **client-side** - the URL fragment is
  **never** sent to the Secutils.dev server. The share link is safe for
  content the user wouldn't want logged, but anyone who receives the link
  can read the source.
- The fragment is bounded by browser/server URL limits (~8 KB practical
  ceiling). Very large (book-length) documents won't fit - have the user
  **Download HTML** and share the file instead, or host the `.md` and use
  `?url=`.
- `?url=` needs a **CORS-enabled** source; `raw.githubusercontent.com` works,
  many web pages do not. On a CORS/network failure the tool shows an error
  toast and stays empty.
- Embedded HTML in the Markdown is rendered as-is. Don't paste untrusted
  Markdown containing `<script>` into a link you're about to share - that's a
  stored-XSS-by-helpfulness risk.
- **KaTeX** and **Mermaid** lazy-load from a CDN on first use and degrade
  gracefully if unreachable (math falls back to literal source, Mermaid to a
  highlighted code block). Exported HTML/PDF with math link the KaTeX CSS
  (and fonts) from the CDN, so math rendering in exports is not byte-for-byte
  offline - same trade-off as the Google Fonts the export already links.
- Markdown emphasis inside math can interfere: because rendering runs after
  the Markdown parser, a formula containing `_` or `*` (e.g. `$a_b$`) may be
  partially consumed as emphasis. Wrap such formulas in display `$$ … $$` or
  escape the characters if a formula renders wrong.
- The CodeMirror editor lazy-loads from `esm.sh`; if the CDN is unreachable
  it falls back to a plain `<textarea>` and all core features still work.
- PDF is a true vector PDF (Paged.js, ~150 KB lazy-loaded), not a screenshot.
