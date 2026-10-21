---
name: markdown-preview
description: >-
  Render and read Markdown in the browser with the Secutils.dev Markdown
  Preview tool. Reading-first: the rendered preview is the default full-width
  view, with on-demand source editing and self-contained HTML/PDF export.
  Supports GitHub-flavored Markdown, Mermaid diagrams, GeoJSON/TopoJSON maps, STL and colored 3MF models, KaTeX math, GitHub
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
default central view; source editing and self-contained HTML/PDF export are
available on demand.

## What it renders

- **GitHub-Flavored Markdown** - tables, task lists, ~~strikethrough~~,
  autolinks.
- **Syntax highlighting** - fenced code blocks via highlight.js, with a
  language label and copy button in exports.
- **Mermaid diagrams** - fenced ```` ```mermaid ```` blocks render to inline
  SVG (pre-rendered, so identical in preview, HTML export, and PDF).
- **GeoJSON and TopoJSON maps** - fenced `geojson` and `topojson` blocks display
  interactive longitude/latitude maps. Drag to pan, use arrow keys, or use the zoom and reset buttons.
  Feature properties appear in hover tooltips. Maps use an OpenStreetMap basemap with a Web Mercator projection.
- **ASCII STL models** - fenced `stl` blocks display interactive 3D models.
  Drag or use arrow keys to rotate. Controls provide zoom, reset, Solid, Wireframe, and Surface angle modes.
  Binary STL is not supported in Markdown fences.
  Solid and Surface angle modes hide facet edges. Overlay controls include rotation, zoom, reset, and fullscreen.
  Pan controls sit beside zoom. Shift-drag or Shift + arrow keys also pan the model without rotation.
- **Copy source** - each map and STL viewer has an overlay button that copies the original fenced source.
- Maps and models retain their SVG or WebGL view and controls in exported HTML without external runtime dependencies.
  PDF exports use the saved static view. Invalid blocks display an error and preserve their source.
- Map basemap tiles require a network connection. Geometry remains visible and interactive without tiles.
  Tile requests go directly from the browser to OpenStreetMap and reveal the displayed geographic area.
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
- **Edit** - edit blocks directly in the preview (see "Editing in the
  preview"), or click **Source** (or `Ctrl/Cmd+E`) to swap to a CodeMirror
  pane; edits update the preview live. `Esc` returns to the preview.
  `Ctrl/Cmd+S` saves in either view (see "Saving").
- **Open** - the **Open** button (or `Ctrl/Cmd+O`), drag-and-drop a file
  onto the page, or just paste (`Ctrl/Cmd+V`) into the empty preview.
  Open or drop an ASCII or binary `.stl` file to create a Markdown `stl` fence.
  Binary STL converts to ASCII STL in the browser. Source editing, sharing, and exports use this converted Markdown.
  Binary color attributes are not retained. Large models can exceed the share URL limit.
  Models with more than 2,000 triangles use WebGL instead of individual SVG triangles.
  Large sources use a plain-text editor to avoid syntax-parser delays. HTML exports retain GPU controls and a static print image.
- **3MF import** - Open or drop a `.3mf` file. A cancellable worker extracts and processes the package locally.
  Core meshes, nested components, repeated instances, build transforms, and declared units are supported.
  Core base-material colors and Materials color groups preserve object defaults and triangle overrides.
  Per-vertex color groups interpolate in sRGB. Standalone surface colors are opaque.
  All 3MF models use WebGL, including small models.
  Optional textures, advanced materials, and slicer-specific paint metadata use the default blue material.
  ZIP and ZIP64 packages are supported within the same size limits.
  Unsupported required extensions, cross-part components, and external relationships produce an error.
  Archive and expanded content are limited to 128 MiB. Rendered geometry is limited to 1,000,000 triangles.
  Worker processing stops after 30 seconds.
  Opening another file or editing the source cancels an earlier import. The **Cancel import** button preserves the current document.
- **3MF source** - Imports create a tool-specific `su-mesh` fence, not an STL fence or a GitHub-compatible model block.
  The JSON body has `version: 1`, base64 Float32 `positions` and `colors`, `count`, bounds, units, instance identities, and warnings.
  Positions contain three separate XYZ vertices per triangle. Colors contain matching sRGB triplets.
  Positions use millimeters in Y-up viewer coordinates. Instance metadata retains the original transform chains.
  The original archive is discarded. Source editing, Markdown downloads, sharing, and Copy source use this representation.
  Copy source copies the JSON fence body. HTML exports preserve interactive colors offline. PDF/print uses a static color image.
  Large models remain available through HTML export even when a share URL is too long.
- **Share** - copies a `tools.secutils.dev/md#<encoded>` URL with the whole
  Markdown round-tripped through the fragment.
- **Export** - **Download HTML** (single self-contained file), **Download
  PDF** (`Ctrl/Cmd+P`, paginated via Paged.js), or **Copy HTML**.
- **Options** (gear) - set the reading width for Preview and exported HTML:
  Comfortable (800px, default), Wide (1100px), Full width, or a custom
  320–2400px width. The same panel controls exported HTML features: table of
  contents, PDF export button, in-page find, and embedded Markdown source.
- **Reading settings in exported HTML** - readers can change the width or
  restore the document default from the toolbar. Their choice lasts for the
  current page visit; reloading restores the width selected at export time.

## Editing in the preview

The rendered preview is editable one block at a time, as in Notion. Every edit
replaces only that block's source text, so the rest of the file stays byte-identical.

- **Task checkboxes** - click to toggle `[ ]` and `[x]` in the source.
- **Block handle** - hover a block or list item to show `+` and a grip in the left margin.
  - `+` adds a block below. On a list item, it adds a sibling item with the same marker.
  - Click the grip to open the actions: **Edit**, **Mark done**, **Copy text**, **Move up**,
    **Move down**, and **Delete**.
  - Drag the grip to move the block. A line shows where it lands. List items move within
    their list. Headings move their whole section past sections of the same level.
- **Format selected text** - select words in one block to show a toolbar: **Bold**, **Italic**,
  **Strikethrough**, **Highlight** (`==text==`), and **Inline code**. Applying a format to text
  that already has it removes the format from the selection. A selection that partly covers
  text with the same format gets one combined run. `Ctrl/Cmd+B` and `Ctrl/Cmd+I` also work.
- **Edit a block** - double-click it, or use **Edit**. The block becomes a small Markdown
  editor. `Esc`, `Ctrl/Cmd+Enter`, or clicking elsewhere commits it and renders it again.
  An unchanged or empty result changes nothing.
- **Add at the end** - click the empty space below the last block.
- **Headings act on sections** - a section runs to the next heading of the same or a higher level.
  - **Mark done** wraps the heading text in `~~...~~` and collapses the section.
    Click a done heading to expand or collapse it. **Mark not done** removes the strikethrough.
  - **Delete section** and **Move section up/down** act on the whole section.
- **Copy text** - copies the rendered text of a block, for example a reply to paste elsewhere.
- **Undo** - `Ctrl/Cmd+Z` and `Ctrl/Cmd+Shift+Z` in the preview undo and redo block edits.
  A deleted block also shows an **Undo** button.
- **Move with the keyboard** - `Alt+Up` and `Alt+Down` move the block under the pointer.
- **Progress** - when the document has task items or a done section, the toolbar shows
  `N of M left`. This counts task items plus the top-level sections (the shallowest heading below `h1`).
- **Hide done items** (Options) - hides checked tasks and done sections.

Block editing is not available for documents over 10 MB.
Those documents still render, and the **Source** view still edits them.

## Saving

- **URL fragment** - after each edit, the page URL fragment is updated with the
  document. A reload keeps your edits, including in tabs opened by `mdpreview`.
- **Save to the file (Chromium browsers)** - open a `.md` file with **Open** or drag and drop.
  The tool then saves edits back to that file automatically after the browser grants write permission.
  The toolbar shows **Saved** or **Unsaved**. Without an open file, `Ctrl/Cmd+S` asks where to save,
  then keeps saving to that file.
- **Conflicts** - if the file changes on disk after the tool read or wrote it, autosave stops
  and the toolbar shows **Changed on disk**. `Ctrl/Cmd+S` asks before it overwrites the file.
- **Other browsers** - `Ctrl/Cmd+S` downloads the `.md`.
- YAML frontmatter and CRLF line endings are kept in saved files, downloads, and the URL fragment.

## Saved model views

STL and 3MF viewers have a **Save current view to source** button (disk icon).
Rotate, pan, zoom, or select a display mode. Then click this button before export.
Preview, HTML, PDF, Markdown downloads, and share links use the saved view.
Later navigation changes the current view until you save again. Reset view restores the saved rotation, pan, and zoom.

View settings are optional JSON metadata on the opening fence:

````markdown
```stl view={"yaw":-0.6,"pitch":0.5,"zoom":1,"panX":0,"panY":0,"mode":"Solid"}
solid model
...
endsolid model
```
````

The same syntax works with `su-mesh` instead of `stl`. The mesh JSON body remains unchanged.
This metadata is specific to Secutils.dev. It controls the viewer, not the model geometry or imported component transforms.

| Setting | Default | Meaning |
| --- | --- | --- |
| `yaw` | `-0.6` | Rotation around the viewer Y-axis, in radians |
| `pitch` | `0.5` | Rotation around the viewer X-axis, in radians, after yaw |
| `zoom` | `1` | Scale multiplier, from `0.1` to `20` |
| `panX` | `0` | Horizontal offset in an 800×400 reference viewport, positive right |
| `panY` | `0` | Vertical offset in the reference viewport, positive down |
| `mode` | `Solid` | `Solid`, `Wireframe`, or `Surface angle` |

Partial settings use defaults for omitted fields. Rotation and pan values must be finite and within ±1,000,000.
Invalid settings display an error and preserve the source.
The save button updates only its model fence and appears in the editable tool preview.
Exported HTML readers can navigate but cannot modify the original Markdown.

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
- **TopoJSON** conversion lazy-loads `topojson-client` from a CDN.
  If the CDN is unavailable, the map displays an error and its source. GeoJSON and STL need no external renderer.
- Markdown emphasis inside math can interfere: because rendering runs after
  the Markdown parser, a formula containing `_` or `*` (e.g. `$a_b$`) may be
  partially consumed as emphasis. Wrap such formulas in display `$$ … $$` or
  escape the characters if a formula renders wrong.
- The CodeMirror editor lazy-loads from `esm.sh`; if the CDN is unreachable
  it falls back to a plain `<textarea>` and all core features still work.
- PDF retains vector text. WebGL models use static PNG images in PDF and print output.
