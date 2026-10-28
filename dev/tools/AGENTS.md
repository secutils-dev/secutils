# Tool apps (`dev/tools/`)

Tool apps deploy as one HTML file with embedded CSS and JS, plus an AI-agent skill beside it.
`dev/tools/deploy.ts` publishes both as Secutils webhook responders on `{{TOOLS_HOST}}` (`tools.secutils.dev`).
MARS is the one bundled app, on its own host: read [guides/mars.md](guides/mars.md) before changing it.
The main CI workflow ignores `dev/tools/**`.
The MARS workflow covers `js/mars/` and the shared foundation with build, typecheck, unit, and search-quality checks.
Run the tool verification and browser gates below locally.

## Contracts

- **Files**: `<name>.html` and `<name>.skill.md`. The filename need not match the URL path.
- **Metadata**: the `su-tool-path`, `su-tool-name`, `su-tool-description`, and `su-tool-promote` `<meta>` tags
  are the source of truth for `deploy.ts`, `scripts/tools-check.ts`, and the marketing site.
  `e2e/tools/registry.ts` repeats those four fields beside its OG and E2E data, so update its row in the same change.
- **Hosts**: write `{{TOOLS_HOST}}` (and `{{MARS_HOST}}` in MARS) wherever a page or skill names its host; `deploy.ts` substitutes them.
  The Plausible script URL is the one literal host ([guides/chrome.md](guides/chrome.md#analytics-plausible)).
- **Dependencies**: CDN-hosted libraries, or an npm package bundled through `data-su-bundle` ([guides/platform.md](guides/platform.md)). Deployment inlines shared styles and glyphs for migrated tools.
- **State**: reload-safe and shareable state lives in the URL fragment, in the shared encoding ([guides/url-state.md](guides/url-state.md)).
- **Browsers**: current Chrome, Firefox, Safari, and Edge. Use a Baseline Newly Available feature when it replaces custom code.
  For an engine that still lacks it, add a fallback only when it is at most 20 lines, needs no polyfill or extra script,
  and the tool keeps working without it; otherwise stay on the existing pattern.
  Limited Availability features are out of scope, except the File System Access API as a feature-detected enhancement in `md.html` and MARS.
- **Page**: `<title>Tool Name | Secutils.dev</title>`, `data-theme="dark"` on `<html>` with a theme toggle, and the shared chrome.
  MARS, ECHO, and Webhook Inspector use the [shared design system](guides/design-system.md).
  `md.html` remains the reference for unmigrated tools.
  Known deviations: `mock-saml-idp.html` (pre-banner chrome), and `index.html` and `certificate-decoder.html` (ES5 syntax).

## Guides

Read every guide whose surface the change touches.

| Surface                                                                                                                   | Guide                                              |
|---------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------|
| `su-tool-*` metadata, promotion, hosts, `@su:responder-script`, `dev/tools/js/` bundles, responder setup                  | [guides/platform.md](guides/platform.md)           |
| URL-fragment state, share links, short links                                                                              | [guides/url-state.md](guides/url-state.md)         |
| `*.skill.md`, `llms.txt`, `robots.txt`, `sitemap.xml`, agent-skills index, `Accept: text/markdown`, header Skill button   | [guides/agent-surface.md](guides/agent-surface.md) |
| `<head>` SEO tags, `<noscript>`, bottom CTA, OG images                                                                    | [guides/seo-og.md](guides/seo-og.md)               |
| Colors, typography, header, buttons, panes, toasts, footer, analytics, responsive rules, `fetchpriority`, exports and PDF | [guides/chrome.md](guides/chrome.md)               |
| Shared CSS or glyphs, MARS chrome, migration of tool styling                                                              | [guides/design-system.md](guides/design-system.md) |
| Privacy and Credits dialogs, popovers, form validation                                                                    | [guides/components.md](guides/components.md)       |
| Tool JavaScript: style, long tasks, busy state, polling                                                                   | [guides/javascript.md](guides/javascript.md)       |
| MARS                                                                                                                      | [guides/mars.md](guides/mars.md)                   |

## Change a tool

1. Edit the HTML. Update the skill in the same change when the edit alters what an agent can do or how it builds a URL.
2. Run `make tools-verify ARGS="dev/tools/<name>.html"`: inline scripts and the responder script parse, and the minifier accepts the page.
3. For URL-state changes, round-trip representative payloads (small, large, Unicode, the default state):
   `decodeState(await encodeState(x)) === x`.
4. For responder-script changes, run `make e2e-standalone-test` when a smoke spec covers the tool
   (`e2e/standalone/md_responder_script_smoke.spec.ts` covers `md.html` and `mars.html`).
   Otherwise run the script under `node:vm` with a stub `secutils`/`Deno.core` for a valid input,
   a malformed input (a clean error response), and a missing input (`null`, so the static body is served).
5. Run `make tools-check`.
6. Deploy when the user asks: `make deploy-tools ARGS="<name>"`. Every run also refreshes the tools-host aggregates.
7. After a deploy, run the tool's spec against the live host: `make e2e-tools-test ARGS="<slug>.spec.ts"`.

Completion: steps 2-5 pass, step 7 passes after a deploy, and the skill, registry row, and README describe the changed behavior.
Report any step you could not run as unverified.

## Add a tool

1. Create two responders, one for the page and one for the skill, and record their IDs in `.env` as
   `SECUTILS_HTML_APP_RESPONDER_ID_<NAME>` and `SECUTILS_HTML_APP_RESPONDER_ID_<NAME>_MD`
   (`<NAME>` is the filename stem in upper snake case). Responder location setup is in [guides/platform.md](guides/platform.md).
2. Write `<name>.html`: metadata ([guides/platform.md](guides/platform.md)), the SEO head ([guides/seo-og.md](guides/seo-og.md)),
   the shared chrome ([guides/chrome.md](guides/chrome.md)), the dialogs ([guides/components.md](guides/components.md)),
   and the Skill button ([guides/agent-surface.md](guides/agent-surface.md)).
3. Write `<name>.skill.md` ([guides/agent-surface.md](guides/agent-surface.md)).
4. Add the `e2e/tools/registry.ts` row and `e2e/tools/<slug>.spec.ts`, starting from `jwt.spec.ts`.
5. For a promoted tool, add an `index.html` card with its JSON-LD `ItemList` entry, a README "Free single-page tools" row,
   and a card in the marketing site's `#free-tools` section (path in `SECUTILS_TOOLS_PROMO_HOME_INDEX`).
   The aggregates regenerate on deploy.
6. Generate the OG pair: `make tools-og ARGS='-g "OG image for <slug>"'`.
7. Run steps 2-7 of "Change a tool".

Completion: `make tools-check` passes, the new spec passes against the deployed tool,
and `og-<slug>.png` and `og-<slug>-light.png` exist in `components/secutils-docs/static/img/og/`.
