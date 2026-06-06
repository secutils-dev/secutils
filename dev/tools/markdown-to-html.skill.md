---
name: markdown-to-html
description: >-
  Convert Markdown to a self-contained HTML page (one file, no external
  assets) or a print-ready PDF using the Secutils.dev Markdown to HTML tool.
  Hand the user https://tools.secutils.dev/md-to-html (optionally with the
  Markdown encoded in the URL fragment for one-click preload), tell them to
  edit in the left pane and watch the live preview, then click **Share**,
  **↓ HTML**, or **↓ PDF**. Trigger when the user asks to "convert this
  markdown to HTML", "render markdown as a printable PDF", "make a
  self-contained HTML page from markdown", "share a rendered markdown
  preview", or anything that names secutils.dev/md-to-html.
---

# Markdown to HTML (Secutils.dev)

In-browser Markdown converter with a live preview, CodeMirror-powered
Markdown syntax highlighting, and three export paths:

1. **Share** - copies a `tools.secutils.dev/md-to-html#<encoded>` URL with
   the entire Markdown round-tripped through the URL fragment.
2. **↓ HTML** - downloads a single self-contained HTML file (every asset
   inlined: fonts, syntax highlighting, optional TOC, theme toggle).
3. **↓ PDF** - opens the browser print dialog against a Paged.js-rendered
   document with proper page numbers, running titles, and break-avoid rules.

Fenced ```` ```mermaid ```` code blocks are rendered to inline SVG diagrams
(GitHub-style). The SVG is pre-rendered and inlined at conversion time, so it
shows up identically in the live preview, the self-contained HTML download, and
the PDF - no Mermaid runtime ships in the exported files.

YAML frontmatter (`title`, `author`, …) is honoured; the `title` field
becomes the downloaded file's `<title>` and filename.

## Inputs

| Field      | Type   | Default  | Notes                                                                      |
|------------|--------|----------|----------------------------------------------------------------------------|
| `markdown` | string | required | Markdown source. YAML frontmatter (`---\ntitle: ...\n---`) is honoured.    |

The page also has an **Options** popover (gear icon) with three checkboxes
that influence the **HTML export**:

| Option                  | Default | Effect on the generated HTML                                                                                |
|-------------------------|---------|-------------------------------------------------------------------------------------------------------------|
| Table of contents       | on      | Floating sidebar generated from `h2`/`h3` headings (only shown when ≥ 3 headings exist).                    |
| PDF export button       | on      | Floating "↓ PDF" button in the top-right that calls `window.print()` against the inlined `@media print` rules. |
| Embed Markdown source   | off     | Appends `<script type="text/markdown" id="su-md-source">…</script>` to the body, retrievable via `document.getElementById('su-md-source').textContent`. |

## Wire format (URL state)

Same canonical format every Secutils.dev tool uses:

```
| 4 bytes uncompressed-length (LE u32) | N bytes raw DEFLATE of UTF-8 markdown |
```

Pipeline: UTF-8 bytes of the raw Markdown → `deflate-raw` → prepend the
4-byte LE u32 of the **uncompressed** length → base64url (`+`→`-`, `/`→`_`,
strip `=`). The state is just the Markdown text itself - no JSON wrapper.

## How to direct the user

If the user wants to interactively edit / preview / share, hand them the
plain URL:

```
https://tools.secutils.dev/md-to-html
```

If you already have their Markdown and want them to land on a pre-filled
page (no copy-paste step), encode and pass it in the fragment. From any
machine with Node ≥ 18:

```bash
node -e '
const zlib = require("node:zlib");
const md = process.argv[1];
const utf8 = Buffer.from(md, "utf8");
const out = Buffer.concat([Buffer.alloc(4), zlib.deflateRawSync(utf8)]);
out.writeUInt32LE(utf8.length, 0);
const enc = out.toString("base64").replace(/\+/g,"-").replace(/\//g,"_").replace(/=+$/,"");
console.log("https://tools.secutils.dev/md-to-html#" + enc);
' '# Hello

This is **markdown**.'
```

Pass the Markdown as a single argv (single-quoted) so newlines, backticks,
and shell metacharacters survive intact. **Always print the full URL** -
the fragment is opaque and dropping a single character breaks decoding.

## Inline alternative (no tool needed)

If the user just wants HTML in chat (not a downloadable file with the
Secutils.dev styling, TOC, and theme toggle), convert it yourself with any
Markdown library: `marked`, `markdown-it`, `pandoc -t html5`, etc. Use the
tool when the user wants a polished, self-contained artefact they can
email, share, or print.

## After producing

If you've handed over the URL, that's the whole interaction - the user
takes it from there in the browser. No follow-up encoding required.

## Caveats

- The Markdown only ever exists **client-side** - the URL fragment
  (everything after `#`) is **never** sent to the Secutils.dev server. The
  share link is therefore safe for content the user wouldn't want logged,
  but anyone who receives the link can read the source.
- Embedded HTML in the Markdown is rendered as-is. Don't paste untrusted
  Markdown that contains `<script>` tags into a tool you're about to share
  with someone - that's a stored-XSS-by-helpfulness risk.
- PDF rendering is paginated by Paged.js (~150 KB lazy-loaded the first
  time you click **↓ PDF**). The PDF is a true vector PDF, not a screenshot.
- The URL fragment is bounded by browser/server URL limits (~8 KB practical
  ceiling). Very large documents (book-length) won't fit; suggest the user
  download HTML and share the file instead.
- The CodeMirror editor lazy-loads from `esm.sh` on first paint. If the
  CDN is unreachable, the page falls back to a plain `<textarea>` - all
  core functionality (live preview, share, export) still works.
- Mermaid is lazy-loaded from a CDN the first time a ```` ```mermaid ````
  block is rendered. If it can't load, the block degrades to a plain
  highlighted code block. Diagrams use a fixed light theme (rendered inside a
  light "figure card") so they read well in both light and dark page themes.
- Mermaid's parser rejects some unquoted node labels - most commonly labels
  in square brackets that contain parentheses, e.g.
  `A[After eating (3x/day)]`. The tool makes a best-effort attempt to fix this
  by auto-quoting such labels (`A["After eating (3x/day)"]`) and re-parsing.
  If a diagram still fails to parse, it renders as an inline error box (with
  the parser message and the original source) instead of breaking the rest of
  the page - so prefer quoting labels that contain special characters yourself.
