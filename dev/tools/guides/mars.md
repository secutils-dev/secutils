# MARS (`mars.html`)

Read before changing MARS: `mars.html`, `mars.skill.md`, `js/mars/`, or `mars/assets/` (paths relative to `dev/tools/`).

MARS (Markdown Annotation & Review System) is the review app extracted from `md.html`. It is the one own-host app
([platform.md](platform.md#hosts-tools_host-mars_host)): served at `/` of `{{MARS_HOST}}` (`mars.secutils.dev`, a
prefix-match `_MARS` responder) with its SKILL.md at `/llms.txt` (an exact-match `_MARS_MD` responder).
It is promoted like any tool, linked by that host. In-app URLs derive from `location.pathname`, so the page works on any host.

## How it differs from the other tools

- **Source layout**: `mars.html` is a thin shell (SEO head, static DOM, the `@su:responder-script`, favicons) plus two
  `gzip-base64` bundles: `mars` (the app, a TypeScript sub-package in `js/mars/`, built by esbuild) and `three-mf`
  (shared with `md.html`). The shell imports the `mars` bundle at startup; `html.mars-boot` hides the body until the
  bundle has injected its CSS.
- **Look**: EUI-styled (Borealis tokens, EUI icons, a header with a docked sidebar like the Web UI), instead of the
  tools chrome: Privacy and Credits sit in the sidebar, and there is no CTA banner or footer. The brand ramp
  (`--mars-*`, `--brand*`) follows the logo and lives in `js/mars/src/ui/tokens.css`. The logo artwork lives in
  `mars/assets/`; the header mark and data-URI favicons in `mars.html` are copies of it.
- **Viewer**: `js/mars/src/doc/viewer.js` is a copy of `md.html`'s live script, minus HTML/PDF export, the HTML view, and
  fragment autosave; `viewer.d.ts` is its host API. A fix to rendering, block editing, formatting, or comments in
  either file lands in both: after porting, diff the shared sections of the two scripts.
- **Storage**: documents live in IndexedDB (database `mars`: `nodes`, `docs`, `meta`). The fragment is a route
  (`#doc=<id>`) or an incoming share, never autosaved state. Every document carries an `originId` that survives sharing;
  opening a link for a known `originId` merges its review threads (by comment `id`, or `author` plus `at`).
  Open existing databases without a requested version. Some browsers retain version 2 from the deferred review-rounds feature.
  Preserve extra stores during rollback. Never downgrade or delete the database to restore the previous UI.
- **Links**: full links are md.html-compatible (`#name=&md=`, plus `&o=<originId>`); folders use
  `#b=<state of a JSON bundle>`. Short links use the md.html scheme ([url-state.md](url-state.md#short-links-mdhtml-mars))
  with a different plaintext (`encodeState` bytes of the bundle JSON), in MARS's own KV, so `md.html` and MARS short
  links are not interchangeable. Client state: `localStorage["mars.shortLinks"]` and `["mars.shortTtl"]`.

## Verify a change

1. In `js/mars/`: `npm test` (codec, tree, and merge logic) and `npm run typecheck`.
2. `npm run serve` builds the deploy-equivalent page and serves it at `http://localhost:7373/` and `/llms.txt`, with the
   responder script on an in-memory KV.
3. From the root: `MARS_BASE_URL=http://localhost:7373 make e2e-tools-test ARGS=mars.spec.ts`.
4. Run steps 2, 4, and 5 of "Change a tool" in [`../AGENTS.md`](../AGENTS.md); for a viewer change, also the affected
   `md.spec.ts` cases.
5. After a deploy, `make e2e-tools-test ARGS=mars.spec.ts` (`MARS_BASE_URL` defaults to `https://mars.secutils.dev`).

Completion: steps 1-4 pass, step 5 passes after a deploy, and a viewer fix is present in both `viewer.js` and `md.html`.
