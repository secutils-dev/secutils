---
name: pdf-extractor
description: >-
  Extract spatial text (liteparse grid projection), structured JSON with
  per-page bounding boxes, or a heuristic Markdown reconstruction (with
  headings, lists, tables, and hyperlinks) from a PDF using the
  Secutils.dev PDF Extractor tool. Optionally runs in-browser OCR for
  scanned PDFs via Tesseract.js. Hand the user
  https://tools.secutils.dev/pdf so they can drop the PDF, click
  **Parse**, switch to the **Text** / **JSON** / **Markdown** tab they
  want, and then **Share** / **Copy** / **Download** the result. PDFs
  are NEVER uploaded -- parsing runs entirely in the user's browser, and
  the share URL carries only the extracted output. Trigger when the user
  asks to "extract text from a PDF", "convert PDF to JSON with bounding
  boxes", "convert PDF to Markdown", "extract tables from a PDF", "OCR
  a scanned PDF in the browser", "get structured PDF output without
  uploading", or anything that names secutils.dev/pdf or run-llama
  liteparse.
---

# PDF Extractor (Secutils.dev)

In-browser PDF parser. Bundles the upstream [liteparse](https://github.com/run-llama/liteparse)
engine, [PDF.js](https://github.com/mozilla/pdf.js) renderer, and
[tesseract.js](https://tesseract.projectnaptha.com/) OCR into one HTML file
(~3 MB inlined). No server-side parsing, no uploads of the PDF bytes.

Four result tabs:

1. **Text** -- liteparse's grid-projected output. Plain UTF-8, preserves
   column / table layout via fixed-width whitespace better than naive
   `pdf.js` text extraction. Suitable as the input to a Markdown converter
   (the page has a one-click "open in Markdown to HTML" handoff).
2. **JSON** -- structured output: `{ pages: [{ page, text, items, boundingBoxes }] }`,
   with per-item rectangles, font sizes, page rotation, and (when OCR ran)
   confidence scores.
3. **Markdown** -- heuristic reconstruction built from the JSON tab's
   spatial data. Lazy on first click and cached after. Detects:
   - **Headings** (`#`, `##`, `###`) from items whose `fontSize` exceeds
     the document-wide body median (1.45x / 1.20x / 1.08x cutoffs).
   - **Bullet lists** (lines starting with `•`, `·`, `-`, `*`, `–`, `—`)
     and **numbered lists** (lines starting with `1.`, `a)`, `iv.`, ...).
   - **Tables** -- runs of at least 3 paragraph-classified lines whose
     left-edge x-anchors line up within +/-5 PDF points across at least
     2 columns become GitHub-flavored markdown tables; first row is the
     header.
   - **Inline bold / italic** from PDF font names (`Bold` / `Black` /
     `Heavy`, `Italic` / `Oblique`).
   - **Hyperlinks** via a separate PDF.js annotation pass over the
     original bytes -- only when the user parsed the PDF locally, NOT
     when hydrating from a shared URL (the bytes are out of scope).
   - **Page breaks** as `---` horizontal rules between every page.

   The "open in Markdown to HTML" handoff button works from this tab too.
4. **Screenshots** -- per-page PNG renders at 150 DPI, generated lazily
   the first time the user clicks the tab. Each page streams in as it
   finishes (PDF.js canvas renderer, no PDFium). Per-page download links
   sit in each figure caption; share / copy / download in the toolbar are
   disabled while this tab is active (no URL state, each page is a file).

Three export paths from the result pane:

- **Share** -- copies a `tools.secutils.dev/pdf#<encoded>` URL with the
  result (Text, JSON, or Markdown, whichever tab is active) round-tripped
  through the URL fragment. Disabled when the payload is over ~64 KB and
  while the heuristic Markdown engine is still computing.
- **Copy** -- copies the active tab's payload to the clipboard.
- **Download** -- saves `<src>.txt`, `<src>.json`, or `<src>.md` depending
  on the active tab.

## Inputs

| Field        | Type                      | Default  | Notes                                                             |
|--------------|---------------------------|----------|-------------------------------------------------------------------|
| PDF file     | binary                    | required | Dropped on the dropzone or chosen via file picker. NOT uploaded.  |
| OCR mode     | `auto`\|`always`\|`never` | `auto`   | Run OCR only on text-sparse pages / always / never.               |
| OCR language | string                    | `eng`    | Tesseract.js language code (`eng`, `deu`, `fra`, `eng+deu`, ...). |

Options live in the gear popover next to **Parse**. Defaults are reasonable
for any Latin-script PDF.

## Wire format (URL state)

The shared canonical encoding every Secutils.dev tool uses:

```
| 4 bytes uncompressed-length (LE u32) | N bytes raw DEFLATE of UTF-8 string |
```

Pipeline: UTF-8 bytes of `JSON.stringify(state)` -> `deflate-raw` -> prepend
the 4-byte LE u32 of the uncompressed length -> base64url (`+` -> `-`,
`/` -> `_`, strip `=`).

Unlike `md-to-html` (which puts the raw Markdown directly in the URL), the
PDF Extractor wraps its state in a JSON envelope because the URL has to
carry both the result and a flag for which tab to open on the destination:

```ts
type SharedState = {
  v: 2;                                    // schema version (v1 still accepted: no 'md'/'m')
  f: 'text' | 'json' | 'md';               // which tab to open
  s: string;                               // source PDF filename (no .pdf)
  t?: string;                              // text body, present when f='text'
  j?: ParseResultJson;                     // structured json, present when f='json'
  m?: string;                              // rendered markdown, present when f='md'
};
```

The `m` payload is the **rendered Markdown text**, not a recipe -- the
heuristic engine is free to evolve between releases, so we share the
finished string so the recipient sees what the sender saw. v1 share links
(no `md` tab) keep working.

Practical cap: ~64 KB of UTF-8 (matching the rest of the toolkit). Larger
results stay in the user's tab but **Share** is disabled with a tooltip
pointing them at **Copy** / **Download** instead.

## How to direct the user

Default: hand them the bare URL and let them drop the file themselves
(this is the common case because PDFs are large and not transferable
through chat):

```
https://tools.secutils.dev/pdf
```

If you already have an **extracted text or JSON** payload from a previous
turn (e.g. you parsed the PDF yourself with another tool) and want to give
the user a pre-filled, shareable view in the browser, encode it into the
fragment using the same wire format as every other Secutils.dev tool.

```bash
# Pre-fill the Text tab with extracted plain text.
node -e '
const zlib = require("node:zlib");
const state = JSON.stringify({ v: 1, f: "text", s: "my-document", t: process.argv[1] });
const utf8 = Buffer.from(state, "utf8");
const out = Buffer.concat([Buffer.alloc(4), zlib.deflateRawSync(utf8)]);
out.writeUInt32LE(utf8.length, 0);
const enc = out.toString("base64").replace(/\+/g,"-").replace(/\//g,"_").replace(/=+$/,"");
console.log("https://tools.secutils.dev/pdf#" + enc);
' "$(cat /tmp/extracted.txt)"
```

```bash
# Pre-fill the JSON tab with a structured liteparse-shaped object.
node -e '
const zlib = require("node:zlib");
const json = JSON.parse(require("node:fs").readFileSync(process.argv[1], "utf8"));
const state = JSON.stringify({ v: 2, f: "json", s: "my-document", j: json });
const utf8 = Buffer.from(state, "utf8");
const out = Buffer.concat([Buffer.alloc(4), zlib.deflateRawSync(utf8)]);
out.writeUInt32LE(utf8.length, 0);
const enc = out.toString("base64").replace(/\+/g,"-").replace(/\//g,"_").replace(/=+$/,"");
console.log("https://tools.secutils.dev/pdf#" + enc);
' /tmp/extracted.json
```

```bash
# Pre-fill the Markdown tab with a rendered Markdown document.
node -e '
const zlib = require("node:zlib");
const md = require("node:fs").readFileSync(process.argv[1], "utf8");
const state = JSON.stringify({ v: 2, f: "md", s: "my-document", m: md });
const utf8 = Buffer.from(state, "utf8");
const out = Buffer.concat([Buffer.alloc(4), zlib.deflateRawSync(utf8)]);
out.writeUInt32LE(utf8.length, 0);
const enc = out.toString("base64").replace(/\+/g,"-").replace(/\//g,"_").replace(/=+$/,"");
console.log("https://tools.secutils.dev/pdf#" + enc);
' /tmp/extracted.md
```

**Always print the full URL** -- the fragment is opaque and dropping a
single character breaks decoding.

If the JSON / text is bigger than ~64 KB, the destination page will refuse
to **re-Share** it (because the fragment can't round-trip something larger
than the source it came in on), but it will still load and the user can
**Copy** / **Download**.

## Inline alternative (no tool needed)

If you have direct access to the PDF bytes and need the text **right now**
(not as a polished, shareable artefact), parse it with any local PDF
library: `pdfjs-dist`, `pdftotext`, `pdfplumber`, `pdf-parse`, or even
`liteparse` itself on Node. Use this tool when the user wants to:

1. Avoid uploading the PDF to anything.
2. Get structured JSON with bounding boxes, not just text.
3. OCR a scanned PDF without standing up Tesseract themselves.
4. Hand the extracted output to a teammate via a single URL.
5. Pipe the text into the [Markdown to HTML](https://tools.secutils.dev/md-to-html)
   converter for a polished export (there is a one-click "Open in Markdown
   to HTML" button right on the result pane).

## Companion: Markdown to HTML

The result pane has a dedicated icon button next to **Download** that opens
the current Text result in the Markdown to HTML tool. Use it when the user
asks "now convert this to a nice PDF / HTML / one-page doc" -- the two
tools share the same URL-fragment wire format for their text payloads, so
the handoff is a single click with no copy/paste.

## After producing

If you've handed over the URL, that's the whole interaction -- the user
takes it from there in the browser. No follow-up encoding required.

## Caveats

- **The PDF bytes only ever exist client-side** -- the URL fragment
  (everything after `#`) is **never** sent to the Secutils.dev server, and
  the dropzone reads the file via `File.arrayBuffer()` directly into a
  Web Worker. The share link is therefore safe for content the user
  wouldn't want logged, but anyone who receives the link can read the
  extracted output.
- **OCR fetches from public CDNs.** When OCR runs, tesseract.js downloads
  its Web Worker (~200 KB) from `unpkg.com` and the requested language
  data (e.g. `eng.traineddata`, ~10 MB) from `tessdata.projectnaptha.com`.
  The PDF content itself is **never** sent to those hosts -- only the
  static asset URLs are requested. Set OCR mode to `Never` in Options to
  guarantee zero third-party contact.
- **First parse is slow.** The bundled engine is ~3 MB inlined; the first
  call to Parse Blob-URLs it and `import()`s the module (one-time ~200 ms
  cost on a modern laptop, longer on mobile). After that it stays in
  memory until the tab is closed.
- **No file conversions.** DOCX / XLSX / HTML / images are rejected at
  the dropzone -- liteparse normally shells out to libreoffice for those
  and there's no browser equivalent.
- **No cmaps shipped.** Latin scripts render perfectly; CJK and some
  specialised PDFs may fall back to substitute glyphs. Bundling cmaps
  (~4 MB more) is a future enhancement once there's user demand.
- **URL state cap is ~64 KB.** Big documents fit easily as Text (a 100-page
  PDF is usually <100 KB of UTF-8) but the JSON variant exceeds the cap
  surprisingly quickly because of per-item bounding boxes. The page
  disables **Share** above the cap and points at **Copy** / **Download**
  instead.
- **Screenshots require the original PDF.** When the user lands via a
  shared URL (which only carries the extracted Text or JSON, never the
  PDF bytes), the Screenshots tab shows a "drop the PDF to enable" prompt
  instead of rendering anything. Rendering also only kicks off on the
  first click of the tab -- pages stream in one at a time so a 50-page
  PDF doesn't pin the main thread before the first page is visible.
- **The Markdown tab is heuristic, not lossless.** It works well for
  documents with clear text-based structure (headings, bullet lists,
  data tables with column-aligned text). It will **miss**:
  - Bordered tables whose cells are not also x-aligned (PDF border
    primitives are not in liteparse's JSON; only text geometry is).
  - Multi-column flow layouts (newspaper-style; columns get glued into
    a single paragraph because line clustering is single-axis).
  - Math, formulae, footnote markers (treated as inline text).
  - Raster images (the Screenshots tab is the right place for those).
- **Links only appear when the PDF is parsed locally.** The hyperlink
  pass is a separate PDF.js annotation extraction that runs against the
  in-memory `pdfFile.bytes`. Shared URLs carry only the rendered
  Markdown string (not the recipe), so link reconstruction is replayed
  on the sender's side at Markdown-render time, and the recipient just
  sees the already-`[text](url)`-wrapped output. If the same JSON is
  rendered on the recipient's side (e.g. they came in via a `f: 'json'`
  share link and then clicked the Markdown tab), the output will be
  link-free.
