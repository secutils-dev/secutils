# mars

The MARS (Markdown Annotation & Review System) app, bundled into `dev/tools/mars.html`
through the `data-su-bundle="mars"` placeholder. `npm run build` writes `dist/mars.js`
(esbuild, ESM, minified). See [`dev/tools/guides/mars.md`](../../guides/mars.md).

Pinned runtime dependencies: `marked` 18.1.1 (the version md.html's unpinned CDN load
resolved to when MARS was extracted) and `fflate` 0.8.3 (same as `three-mf`).

```bash
npm ci
npm test            # codec, tree, and merge logic (node --test)
npm run typecheck
npm run serve       # http://localhost:7373/ (+ /llms.txt), short links on an in-memory KV
```

`src/doc/viewer.js` is a copy of md.html's live script; `src/doc/doc.css` copies its
preview styles. `src/ui/icons.ts` is generated from EUI's SVGs; to add an icon, extend the
`names` map and re-run from the repo root (needs `components/secutils-webui/node_modules`):

```bash
node -e '
const fs = require("fs"), d = "components/secutils-webui/node_modules/@elastic/eui/lib/components/icon/svgs";
const src = "dev/tools/js/mars/src/ui/icons.ts", file = fs.readFileSync(src, "utf8");
const names = { /* camelCaseName: "eui_file_name" */ };
const lines = Object.entries(names).map(([k, f]) => `    ${k}: ${JSON.stringify(fs.readFileSync(`${d}/${f}.svg`, "utf8").replace(/\s+/g, " ").replace(/^.*?<svg[^>]*>/, "").replace(/<\/svg>.*$/, "").trim())},`);
fs.writeFileSync(src, file.replace("} as const;", `${lines.join("\n")}\n} as const;`));
'
```

The logo lives in `dev/tools/mars/assets/` (app icon, favicon, wordmark, and the PNG
favicons rendered from them). `mars.html` inlines copies: the header mark (the wordmark's
planet) and the favicon data URIs. The `--mars-*` / `--brand*` tokens in
`src/ui/tokens.css` follow the logo's reds.
