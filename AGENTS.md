# AGENTS.md

## Dependency upgrades

This repo's dependency surface spans two Cargo workspaces (root + the `components/retrack`
submodule), four NPM packages (`components/secutils-webui`, `components/secutils-docs`,
`components/retrack` + `components/retrack/components/retrack-web-scraper`, the e2e harness
and the workspace-root `package.json`), six Dockerfiles, an Ory Kratos server image, and
the `playwright-core` / `@playwright/test` / `playwright-python` triple. A naive "bump
everything in one commit" produces an unreviewable diff and an undebuggable failure mode.
Always upgrade in **eleven sequential stages**, each individually committable and verifiable.

### Recommended upgrade order

The order is dictated by data flow: the retrack submodule is consumed as a path-dependency
by the root crate, so anything that touches the retrack Rust API must land before the
parent re-pins it; Node and `playwright-core` versions must be bumped before the Dockerfile
rebuilds (otherwise `npm ci` in the runtime stage validates against a stale lock); Kratos
sits between the auth e2e flows and the webui's `@ory/kratos-client-fetch` and benefits
from being upgraded as a coupled pair.

1. **Retrack Rust crates** — `components/retrack/Cargo.toml`,
   `components/retrack/components/retrack-types/Cargo.toml`,
   `components/retrack/benches/js-runtime-perf/Cargo.toml`. See
   `components/retrack/AGENTS.md` for the in-submodule recipe (insta snapshots,
   `.sqlx/` cache, perf harness).
2. **Retrack `.nvmrc`** — bump the Node major; mirror in every `engines.node` and
   `@types/node` ^M.x inside the submodule.
3. **Retrack NPM packages** — submodule root + `retrack-web-scraper`. **Pin the
   `playwright-core` exact version here** — every other consumer (webui, e2e, the
   `playwright-python` git ref baked into `Dockerfile.web-scraper-camoufox`) must be moved
   to the same minor in their respective stages.
4. **Retrack Docker base images** — `Dockerfile`, `Dockerfile.web-scraper`,
   `Dockerfile.web-scraper-camoufox`. UPX, Camoufox triple, `playwright-python` git ref.
5. **Kratos** — bump `oryd/kratos` server image in `dev/docker/docker-compose.yml` **and**
   `@ory/kratos-client-fetch` in `components/secutils-webui/package.json` together. Read
   the Ory release notes for any registration/login/recovery flow schema changes; verify
   end-to-end against `e2e/tests/registration.spec.ts`.
6. **Root Rust crates** — `Cargo.toml`, `components/secutils-jwt-tools/Cargo.toml`,
   `benches/js-runtime-perf/Cargo.toml`. **First** update the retrack submodule pointer
   to the SHA you committed in stages 1–4 (`cd components/retrack && git pull` then
   `git add components/retrack` from the parent), then `cargo update`.
   Refresh `.sqlx/` against the dev Postgres.
7. **Root `.nvmrc`** — bump to the same Node major as stage 2; mirror in `engines.node`
   and `@types/node` of all four root-level `package.json` files (root, secutils-webui,
   secutils-docs, e2e). Refresh all four lockfiles.
8. **`components/secutils-webui` NPM packages** — read the EUI / React / Parcel release
   notes (EUI majors and Parcel resolver behaviour have repeatedly forced workarounds,
   see "What to watch for" below). Re-pin `playwright-core` to the same exact version as
   stage 3.
9. **`components/secutils-docs` NPM packages** — Docusaurus majors change config schemas
   (e.g. `siteConfig.markdown.hooks.onBrokenMarkdownLinks` migration in 3.10), and
   `docusaurus-plugin-llms` defaults change between minors. Verify `llms.txt` /
   `llms-index.txt` and the per-page `.md` companions resolve, and that the Nginx config
   serves them with the right `Content-Type`.
10. **Root Docker base images** — `Dockerfile`, `Dockerfile.docs`, `Dockerfile.webui`. UPX,
    distroless runtime, `nginx-unprivileged`. Re-pin SHA256 manifest digests with
    `./dev/scripts/docker-pin-digests.sh`. Rebuild the e2e stack and curl-smoke each
    service.
11. **`e2e/` harness** — bump `@playwright/test` to match the `playwright-core` from
    stages 3 & 8 (within the same minor), refresh ESLint / TypeScript-ESLint / globals.
    `npx playwright install chromium`. Run **all three test suites**: standalone
    (`make e2e-standalone-test`), full e2e (`make e2e-test`), docs screenshots
    (`make docs-screenshots`).

Do **not** reorder. The most common mistake is bumping the root Rust crates (stage 6)
before the retrack submodule pointer is updated — `cargo update` will then either downgrade
the workspace, or fail to compile because the retrack code on disk has already been
upgraded but its public types now mismatch what the parent expects.

### Stage-by-stage verification

Each stage has a hard verification gate before commit. Use the matching `make` target;
do not skip steps even when the previous stage was green.

| Stage              | Verify                                                                                                                                                                                                         |
|--------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| 1 (retrack Rust)   | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && make perf ANALYZE=1 PERF_ITERATIONS=20 PERF_WARMUP=5`                                                                         |
| 2 (retrack Node)   | `npm install && npm run lint --ws --if-present && npm test --ws --if-present && npm run build --ws --if-present`                                                                                               |
| 3 (retrack NPM)    | same as stage 2                                                                                                                                                                                                |
| 4 (retrack Docker) | `make docker-scraper && make docker-scraper-camoufox && make docker-api` (in retrack)                                                                                                                          |
| 5 (Kratos)         | `make e2e-up BUILD=1 && make e2e-test ARGS="tests/registration.spec.ts"`                                                                                                                                       |
| 6 (root Rust)      | `cargo +nightly fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo sqlx prepare --check && cargo test`                                                                              |
| 7 (root Node)      | `npm --prefix components/secutils-webui run build && npm --prefix components/secutils-docs run build` (and lint/test in each)                                                                                  |
| 8 (webui NPM)      | `npm --prefix components/secutils-webui run lint && npm --prefix components/secutils-webui run test && npm --prefix components/secutils-webui run build && npm --prefix components/secutils-webui run analyze` |
| 9 (docs NPM)       | `npm --prefix components/secutils-docs run typecheck && npm --prefix components/secutils-docs run build` — then check the build output for `llms.txt`, `llms-index.txt`, and at least one per-page `.md`       |
| 10 (root Docker)   | `make docker-api && make docker-webui && make docker-docs && make e2e-up BUILD=1` then curl-smoke `/api/status`, `/`, `/docs/`                                                                                 |
| 11 (e2e)           | `make e2e-standalone-test && make e2e-test && make docs-screenshots` (full suites — partial runs miss DNS / network regressions, see below)                                                                    |

### What to watch for

#### Rust (stages 1 & 6)

- **`deno_core`** bumps invalidate the `js_runtime::tests::can_access_deno_apis` snapshot
  (same as in retrack) and may pin a transitive `deno_error` patch version that needs
  matching in the workspace `Cargo.toml` (e.g. 0.7.1 vs 0.7.3).
- **`sqlx`** macros validate against `.sqlx/`. After any query change or `sqlx` bump:
  ```bash
  docker compose -f dev/docker/docker-compose.yml up -d secutils_db
  cargo sqlx prepare
  ```
  CI runs `cargo sqlx prepare --check` and fails when the cache drifts.
- **`serde_json/arbitrary_precision`** is enabled by `secutils` and propagates via Cargo
  feature unification to the path-dependent `retrack-types`. `retrack-types`'s snapshots
  were recorded without the feature, so `cargo test --workspace` from the **root** repo
  will show snapshot diffs (the values become `serde_json::private::Number` instead of
  numeric literals). This is a known latent issue; CI runs `cargo test` (not
  `--workspace`) so it never trips. Do not "fix" it by re-recording the retrack snapshots
  against the unified feature set — that breaks retrack's own CI.

#### Node / NPM (stages 2, 3, 7, 8, 9, 11)

- **Node 24 removed `--experimental-global-webcrypto`** — `globalThis.crypto` is now
  always present. Removing the execArgv flag in `worker.ts` is mandatory; **also**
  `delete (globalThis as { crypto?: unknown }).crypto;` inside the sandboxed user-script
  worker so user code cannot reach the host's `WebCrypto` API.
- **ESLint 10** ships `preserve-caught-error` (rethrow with `{ cause: err }`) and
  `eslint-plugin-import@2.32.0` does not yet support v10 as a peer. The whole project is
  pinned to **ESLint 9.x** until plugin support catches up. Do not bump `eslint` /
  `@eslint/js` past `^9` in any leaf `package.json`.
- **TypeScript 6** deprecates `compilerOptions.baseUrl`. Remove it and migrate any `paths`
  entries to direct relative imports. The root project is pinned to **TS 5.9.x** because
  Docusaurus and EUI's TS consumers have not validated v6 yet.
- **`eslint-plugin-react-hooks` 7.x** rejects `useCallback(debounce(...))` as improper
  memoization. Use `useMemo(() => debounce(...), [])` instead.
- **`@peculiar/x509` v2 + Parcel.** v2 transitively pulls `@peculiar/utils` which uses
  `package.json#exports` subpaths (`./bytes`). Parcel 2.16's default resolver does not
  honour `exports` subpaths and fails with `Failed to resolve '@peculiar/utils/bytes'`.
  Two options: (a) keep `@peculiar/x509` pinned to `^1.x`, or (b) explicitly enable
  Parcel's `exports` resolution via:
  ```json
  "@parcel/resolver-default": { "packageExports": true }
  ```
  in the leaf `package.json`. The webui takes option (a) for now to avoid the
  `reflect-metadata` polyfill v2 requires.
- **`http-proxy-middleware` v4 is ESM-only**, so it cannot be required from the CommonJS
  `.proxyrc.ts`. Stay on v3 for the Parcel dev-server proxy.
- **EUI majors** read the entire CHANGELOG. Common patterns: token renames in CSS-in-JS
  (`euiTheme.colors.*`), new required props on data-grid, default ARIA-label changes that
  break `getByRole({ name: ... })` selectors in e2e tests.
- **`playwright-core` and `@playwright/test` must share a minor.** The webui pins
  `playwright-core` (used at runtime to render preview), retrack pins it for the scraper,
  and the e2e harness pins `@playwright/test`. They drive the same Chromium and the same
  CDP protocol — a minor mismatch surfaces as "browser closed unexpectedly". After
  bumping, run `make e2e-standalone-test` first; the codegen smoke test detects breaking
  changes to Playwright's `--target` boilerplate before they corrupt the webui's script
  transformer.

#### Docusaurus (stage 9)

- **Nginx `types {}` in server scope replaces, not merges.** When adding `text/markdown
  md;` to serve the per-page companions, you must `include /etc/nginx/mime.types;` first
  in the same `server { … }` block, otherwise `.txt` (and everything else) regresses to
  `application/octet-stream`. The per-page `.md` companions are `text/markdown`. `llms.txt`
  and `llms-index.txt` are **also** served as `text/markdown` despite the `.txt` extension
  (they are llmstxt.org-format markdown files, and the promo site's `Accept: text/markdown`
  content-negotiation 302 lands here -- Cloudflare's "Markdown for Agents" check at
  isitagentready.com rejects a `text/plain` final response). Achieved with a regex
  `location ~ ^/docs/llms(-index)?\.txt$ { types { } default_type text/markdown; }`
  override that empties the inherited MIME map for that one location so `default_type`
  wins. **The `/docs/` prefix matters**: the production Traefik route for the public
  `secutils.dev/llms.txt` (and `/llms-index.txt`) URLs carries an `addPrefix: /docs`
  middleware, so by the time the request reaches the docs nginx the URI is
  `/docs/llms.txt`. A bare `location = /llms.txt` will silently never match -- if you
  smoke-test this from inside the pod (e.g. `kubectl exec ... -- wget /llms.txt`), curl
  the **prefixed** path instead.

#### Docker (stages 4 & 10)

- **Re-pin SHA256 digests on every bump** with `./dev/scripts/docker-pin-digests.sh`
  (root) or `./dev/scripts/docker-pin-digests.sh` inside the retrack submodule. The
  scripts read `FROM image:tag@sha256:...`, drop the digest, query
  `docker buildx imagetools inspect`, and rewrite. They always re-pin, even when the
  tag is unchanged — rolling tags drift between runs.
- **Disk pressure during the Rust image build is real** — the secutils API image
  compiles the full workspace from scratch when the BuildKit cache is cold. If the build
  fails with `No space left on device`, run `make docker-prune` (which prunes both
  dangling images and BuildKit cache) and retry.
- See the retrack AGENTS.md for the workspace-layout `npm ci` gotcha and the Camoufox
  triple — those rules apply identically when bumping the root images that depend on
  retrack's runtime images.

#### Submodule pointer & commit hygiene

- Stage 6 is the **only** stage that updates the retrack submodule pointer. Always do
  `git submodule update --remote components/retrack` (or pull manually inside the
  submodule), then `git add components/retrack` from the parent before running the rest
  of stage 6's verification. Forgetting to commit the pointer leaves the parent referring
  to a pre-upgrade SHA and CI re-runs the old code.
- The repo enforces **conventional commits** via husky `commit-msg`. Use
  `chore(deps): ...` for dependency-only commits, `chore(docker): ...` for image re-pins
  and `chore(submodule): ...` for the retrack pointer bump. Commitlint major bumps
  (e.g. v20 → v21) only change Node minimum; the existing config keeps working.
- **The performance harness is advisory** (see "Tuning" below). A regression after a
  `deno_core` / `tokio` / `reqwest` bump is informational; CI never fails on it.

## End-to-End tests (`e2e/`)

### Overview

E2E tests use **Playwright** (`@playwright/test`) and run against the full application stack
(API, Web UI, Kratos auth, Retrack, Postgres) served via Docker Compose at `http://localhost:7171`.

Tests live in `e2e/tests/` and are named `*.spec.ts`.

### Running tests

```bash
# Start the full e2e stack (all services in Docker)
make e2e-up              # add BUILD=1 to rebuild images

# Run all tests
make e2e-test

# Run a specific test file
make e2e-test ARGS="tests/registration.spec.ts"

# Run in headed mode (opens a browser)
make e2e-test ARGS="--headed"

# Open the Playwright UI runner
make e2e-test ARGS="--ui"

# View the HTML report after a run
make e2e-report

# Tear down the stack
make e2e-down

# Run standalone tests (no Docker stack needed, e.g. codegen smoke tests)
make e2e-standalone-test
```

### Standalone tests (`e2e/standalone/`)

Standalone tests validate tooling and transformers against the currently installed Playwright
version. They do **not** require the Docker application stack - only an installed browser
(`npx playwright install chromium`).

Tests live in `e2e/standalone/` and use `playwright.standalone.config.ts`. Run them with:

```bash
make e2e-standalone-test
```

The codegen transformer smoke test spawns `npx playwright codegen` to capture the current
boilerplate format and verifies the web UI's script transformer can handle it. This catches
breaking changes to codegen output when Playwright is upgraded.

### Debugging flaky tests

To check whether a test is reliably passing, run it in a loop. Both `e2e-test` and
`docs-screenshots` have loop variants that accept the same `ARGS` plus a `RUNS` count
(default 10):

```bash
# Run a specific e2e test 20 times
make e2e-test-loop ARGS="tests/registration.spec.ts" RUNS=20

# Run docs screenshot tests 20 times
make docs-screenshots-loop ARGS="docs/csp.spec.ts" RUNS=20
```

Each run streams `PASS` / `FAIL` to the terminal. On failure, the full Playwright log is
written to `/tmp/e2e-loop-results/run-N.log` and any failure screenshots / traces are
copied to `/tmp/e2e-loop-results/artifacts-run-N/`.

**When the agent cannot run tests directly** (e.g. due to sandbox restrictions on the
Playwright browser binary), ask the user to run the loop command and share the results.
The agent can then read the log files and failure screenshots directly from
`/tmp/e2e-loop-results/` to diagnose failures:

```bash
# User runs this, then the agent reads /tmp/e2e-loop-results/ to debug
make docs-screenshots-loop ARGS="docs/csp.spec.ts" RUNS=20
```

### Locator strategy

Choose locators in this order of preference. Prefer semantic, user-visible locators that
mirror how a real user perceives the page.

1. **`getByRole`** - first choice for buttons, headings, links, and other ARIA-role elements.
   Use `name` to disambiguate and `exact: true` when the label is a substring of another
   element's label.

   ```typescript
   page.getByRole('button', { name: 'Sign up', exact: true });
   page.getByRole('heading', { name: 'Welcome', level: 2 });
   ```

2. **`getByPlaceholder`** - preferred for form inputs that carry placeholder text. Use
   `exact: true` when a shorter placeholder is a prefix of another (e.g. "Password" vs
   "Repeat password").

   ```typescript
   page.getByPlaceholder('Email');
   page.getByPlaceholder('Password', { exact: true });
   ```

3. **`getByText`** - fallback for elements that have visible text but no clear ARIA role or
   placeholder (e.g. menu items, labels).

   ```typescript
   page.getByText('Sign out');
   ```

4. **Raw `locator` (CSS/XPath)** - last resort for cases where no semantic locator is
   practical, such as checking that *any* form control is present without caring about exact
   text.

   ```typescript
   page.locator('input[name="password"], input[name="identifier"], form');
   ```

### Timeouts

The Docker-based stack (Kratos auth + API + Web UI) can be slow to respond, especially on
the first load. Use explicit timeouts on visibility and URL assertions:

- **`toBeVisible({ timeout: 15000 })`** - for elements that depend on the page fully
  rendering or an auth redirect completing.
- **`toHaveURL(pattern, { timeout: 30000 })`** - for navigations that involve server-side
  processing (registration, login).

Do not add timeouts to assertions that follow an already-awaited element on the same page.

### Test structure

- Group related tests with `test.describe`.
- Use `test.beforeEach` for setup that must run before every test in the group (e.g., cleaning
  up test users via the API).
- Keep one test file per feature area (e.g. `registration.spec.ts`, `app.spec.ts`).

### Test data setup and cleanup

Tests that create server-side state (users, resources) must clean up in `beforeEach` so each
run starts from a known state. Use `request` (Playwright's built-in API context) to call
internal API endpoints directly:

```typescript
test.beforeEach(async ({ request }) => {
  await request.post('/api/users/remove', {
    headers: { Authorization: `Bearer ${OPERATOR_TOKEN}` },
    data: { email: EMAIL },
  });
});
```

Define credentials and operator tokens as module-level constants at the top of the file.

### Verifying backend state from tests

When UI assertions alone are not enough, make API calls within the test to verify
server-side state through `page.request`:

```typescript
const stateResponse = await page.request.get('/api/ui/state');
expect(stateResponse.ok()).toBeTruthy();
const state = await stateResponse.json();
expect(state.user.email).toBe(EMAIL);
```

### Assertion Patterns

| What to check        | Assertion                                      |
|----------------------|------------------------------------------------|
| Element is on screen | `await expect(el).toBeVisible({ timeout: … })` |
| Current URL          | `await expect(page).toHaveURL(/pattern/)`      |
| Page title           | `await expect(page).toHaveTitle(/pattern/)`    |
| API response OK      | `expect(response.ok()).toBeTruthy()`           |
| JSON field exists    | `expect(body).toHaveProperty('key')`           |
| JSON field value     | `expect(body.field).toBe(value)`               |

### Code Style

The e2e project uses ESLint + Prettier with these key rules:

- Max line length: **120** characters (strings and template literals exempt).
- Use **`type` imports** (`import type { … }`) where possible - enforced by
  `consistent-type-imports`.
- Import order: builtins, externals, internals, then parent/sibling/index - alphabetized,
  separated by blank lines.
- No unused variables or expressions.

---

## Docs screenshot tests (`e2e/docs/`)

### Overview

Docs screenshot tests generate screenshots used in the documentation site
(`components/secutils-docs/`). Each test file in `e2e/docs/` corresponds to a guide topic
(e.g. `csp.spec.ts`, `webhooks.spec.ts`, `digital_certificates.spec.ts`,
`web_scraping.spec.ts`, `export_import.spec.ts`, `home.spec.ts`, `secrets.spec.ts`,
`user_scripts.spec.ts`). Screenshots are saved directly into
`components/secutils-docs/static/img/docs/guides/<topic>/`.

```bash
# Run all docs screenshot tests
make docs-screenshots

# Run a specific file
make docs-screenshots ARGS="docs/csp.spec.ts"

# Run a single test by name
make docs-screenshots ARGS="docs/csp.spec.ts -g 'test a content security policy'"
```

### Shared helpers (`e2e/helpers.ts`)

All docs tests import from `helpers.ts`. Key exports:

| Helper                                      | Purpose                                                                                                                                 |
|---------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------|
| `ensureUserAndLogin(request, page)`         | Remove existing user, register a fresh one, and log in.                                                                                 |
| `goto(page, url)`                           | Navigate, inject stability CSS, and patch `page.screenshot()` for determinism.                                                          |
| `highlightOn(locator)`                      | Add a red dashed outline around an element for visual emphasis.                                                                         |
| `highlightOff(locator)`                     | Remove the highlight outline.                                                                                                           |
| `dismissAllToasts(page)`                    | Dismiss every visible toast notification (iterate all, not just one).                                                                   |
| `pinEntityTimestamps(json)`                 | Replace `createdAt`/`updatedAt` (and `scheduledAt`/`lastRanAt` for scheduled trackers) with `FIXED_ENTITY_TIMESTAMP` in a JSON value.   |
| `fixEntityTimestamps(page, pattern)`        | Set up a route handler that pins timestamps in GET JSON responses matching `pattern`.                                                   |
| `fixResponderRequestFields(page)`           | Intercept responder request history API and pin `createdAt`/`clientAddress` to fixed values.                                            |
| `fixCertificateTemplateValidityDates(page)` | Pin `notValidBefore`/`notValidAfter` to fixed dates while preserving their duration.                                                    |
| `fixTrackerResourceRevisions(page)`         | Stabilize tracker revision history: strip URL query strings, normalize webhook subdomains, compute deterministic sizes, fix timestamps. |
| `fixTrackerExecutionLogs(page)`             | Intercept tracker execution log responses and pin `startedAt`/`finishedAt`/phase durations to fixed values.                             |
| `fixTrackerHealthDots(page)`                | Intercept tracker health summary (`logs_summary`) responses and pin timestamps to fixed values for stable health dot screenshots.       |

### Screenshot stability

Screenshots must be **byte-identical** across runs. The stability system has multiple layers
that work together automatically when using `goto()`:

#### Automatic stabilization (handled by `goto()` and `patchPageScreenshot`)

These apply to every screenshot without any test-level code:

1. **CSS injection** - `goto()` injects a `<style>` tag after navigation that:
   - Disables all CSS animations and transitions (`animation-duration: 0s; transition-duration: 0s`).
   - Forces greyscale anti-aliasing (`-webkit-font-smoothing: antialiased; text-rendering: geometricPrecision`) - reduces font rendering variance from ±8 to ±1.
   - Forces icon buttons and toggle switches into GPU compositing layers (`.euiButtonIcon, .euiSwitch__body { will-change: transform }`) - reduces SVG/toggle rendering variance from ±24 to ±1.
   - Hides Monaco editor non-deterministic elements (cursor layer, minimap, decorations overview ruler, scroll decoration).
   - Hides the system text caret (`caret-color: transparent`).
   - Hides scrollbars (`::-webkit-scrollbar { width: 0; height: 0 }`).

2. **Pre-screenshot stabilization** - `waitForStableUiBeforeScreenshot()` runs before every
   `page.screenshot()` call and:
   - Waits for `domcontentloaded` and `networkidle` (with 5 s timeout).
   - Waits for all EUI icons to finish loading (`.euiIcon[data-is-loading="true"]`).
   - Waits for all web fonts to reach `loaded` status (`document.fonts.status`).
   - Normalizes webhook subdomain URLs in the DOM - replaces user-specific subdomains
     (e.g. `handle.webhooks.localhost:7171`) with a stable value (`docs.webhooks.`) everywhere
     they appear: `input`/`textarea` `.value` properties, and all text nodes in the body
     (table cells, flyout help text, link text, code blocks, data grid popovers, etc.).
     **The `href` attribute of links is never modified** so tests can still read it for
     navigation after a screenshot.
   - Waits three animation frames for layout/paint/composite to settle.

3. **Sticky-pixel screenshot stabilization** - `stabilizeScreenshot()` runs after
   every `page.screenshot()`.  Before the screenshot is taken, the existing file on disk
   (if any) is saved as a byte buffer.  After capturing, both the reference and new PNGs
   are decoded to raw RGBA pixels with `pngjs` (`PNG.sync.read`).  If every channel value
   in the new image is within ±1 of the reference (`MAX_CHANNEL_DIFF`), the image has
   not meaningfully changed - the original reference bytes are written back verbatim,
   producing zero diff.  This absorbs non-deterministic sub-pixel anti-aliasing jitter
   from Chromium's GPU compositor between browser sessions.  When any pixel genuinely
   differs (channel diff > 1) or the dimensions changed, the new Playwright file is kept
   as-is and becomes the new baseline for future runs.

#### Test-level stabilization (must be added per test/describe)

Each source of dynamic data needs explicit stabilization in the test code:

- **Timestamps / dates**: Intercept the API response with `page.route()` and replace
  dynamic timestamps with `FIXED_ENTITY_TIMESTAMP` (epoch `1740000000`, renders as
  "February 19, 2025" - deliberately >3 days old so the UI shows an absolute date
  instead of a relative string like "a few seconds ago").
- **Client addresses**: Pin to a fixed value like `172.18.0.1:12345`.
- **CSP nonces**: Intercept responses and replace rotating nonces with a fixed value
  (e.g. `nonce-m0ck`).
- **URL query strings**: Strip random cache-buster parameters from resource URLs.
- **Webhook subdomains**: Automatic - `waitForStableUiBeforeScreenshot` replaces every
  `{handle}.webhooks.` occurrence in the DOM with `docs.webhooks.` before each screenshot.
  No test-level code is needed for the URL displayed in grids, help text, or code blocks.
  However, if the "Add responder" flyout is open and a screenshot is taken, the
  auto-generated random subdomain prefix in the **Subdomain prefix** input must be cleared
  explicitly in the test: `await flyout.getByLabel('Subdomain prefix').clear()`.
- **Cryptographic output** (JWK values, key exports): Replace dynamic fields via
  `element.evaluate()` after the UI renders them.
- **Home page summary**: Intercept `/api/ui/home/summary` and call `pinEntityTimestamps()`
  on `recentItems` to avoid relative time strings.
- **Tracker execution logs**: Intercept `*/logs` responses and pin `startedAt`/`finishedAt`
  and phase durations with `fixTrackerExecutionLogs(page)`.
- **Tracker health dots**: Intercept `*/logs_summary` responses and pin timestamps with
  `fixTrackerHealthDots(page)`.

General pattern for stabilization - intercept with `page.route()`, call `route.fetch()`
to get the real response, mutate the JSON, then `route.fulfill({ response, json })`:

```typescript
await page.route('**/api/some/endpoint', async (route) => {
  const response = await route.fetch();
  const json = await response.json();
  json.dynamicField = 'fixed-value';
  await route.fulfill({ response, json });
});
```

When a `page.route()` handler may receive non-array responses (e.g. POST refresh vs GET
list), always guard with `if (!Array.isArray(json))` before iterating.

#### Clipped screenshots with tooltips

When screenshots are clipped to a bounding box (e.g. tooltip + section), round coordinates
to whole pixels and use generous padding to absorb sub-pixel layout jitter:

```typescript
const PAD = 16;
const x = Math.floor(Math.min(sectionBox.x, tooltipBox.x)) - PAD;
const y = Math.floor(Math.min(sectionBox.y, tooltipBox.y)) - PAD;
const right = Math.ceil(Math.max(sectionBox.x + sectionBox.width, tooltipBox.x + tooltipBox.width)) + PAD;
const bottom = Math.ceil(Math.max(sectionBox.y + sectionBox.height, tooltipBox.y + tooltipBox.height)) + PAD;
await page.screenshot({ path, clip: { x, y, width: right - x, height: bottom - y } });
```

### Debugging screenshot instability

When screenshots differ between runs, use the comparison tooling to diagnose:

```bash
# Run docs screenshots twice and diff all PNGs (pixel + byte level)
make docs-screenshots-diff
# Or for a single spec file:
make docs-screenshots-diff ARGS="docs/csp.spec.ts"

# Analyze diffs with detailed per-file report (pixel counts, regions, categories)
make docs-screenshots-analyze
```

The tools output to `/tmp/screenshot-diff/`:

| Path                     | Contents                                                     |
|--------------------------|--------------------------------------------------------------|
| `run-a/`, `run-b/`       | PNG snapshots from each run                                  |
| `diffs/`                 | ImageMagick visual diff images (red = changed pixels)        |
| `analysis/`              | Python-annotated diff images with bounding boxes             |
| `report.txt`             | Summary with per-file pixel diff counts and byte sizes       |
| `analysis-report.json`   | Detailed JSON: pixel counts, bounding boxes, diff categories |
| `run-a.log`, `run-b.log` | Full Playwright output from each run                         |

**Workflow for diagnosing instability:**

1. Run `make docs-screenshots-diff` to produce two runs of screenshots.
2. Run `make docs-screenshots-analyze` to get a detailed report.
3. Check the report categories:
   - `Byte-identical` - no action needed.
   - `Byte-diff only (0 pixel diffs)` - DEFLATE compression non-determinism (should be
     resolved by `reEncodePngDeterministic`; if it re-appears, check for PNG chunk changes).
   - Files with pixel diffs > 0 - need investigation (see below).
4. For files with pixel diffs, run a **deep pixel analysis** to locate the exact element:
   ```python
   # In Python (or inline via shell):
   from PIL import Image
   a = Image.open('/tmp/screenshot-diff/run-a/<file>').convert('RGBA')
   b = Image.open('/tmp/screenshot-diff/run-b/<file>').convert('RGBA')
   for i, (pa, pb) in enumerate(zip(a.tobytes(), b.tobytes())):
       if pa != pb:
           px = (i // 4) % a.size[0]; py = (i // 4) // a.size[0]
           print(f'({px},{py}) {"RGBA"[i%4]}: {pa}->{pb} delta={pb-pa}')
   ```
5. Crop the diff region (`Image.crop()`) and view it to identify the UI element.
6. Apply the appropriate fix from the troubleshooting table.
7. Use the loop command to verify a fix is stable:
   ```bash
   make docs-screenshots-loop ARGS="docs/csp.spec.ts" RUNS=10
   ```

**Expected residual instability:** With sticky-pixel stabilization, all 159 screenshots
should be byte-identical across runs.  If new screenshots are added without an existing
reference file on disk, the first run establishes the baseline; subsequent runs converge.

**Common instability patterns and their solutions:**

| Symptom                     | Likely Cause                                 | Fix                                                                     |
|-----------------------------|----------------------------------------------|-------------------------------------------------------------------------|
| Byte-diff but no pixel diff | PNG DEFLATE non-determinism or ±1 AA jitter  | `stabilizeScreenshot()` (automatic - restores reference file)           |
| Text changes between runs   | Relative timestamps ("a few seconds ago")    | `fixEntityTimestamps()` or `pinEntityTimestamps()`                      |
| URL segments differ         | User-specific webhook subdomain              | Automatic DOM normalization in `waitForStableUiBeforeScreenshot`        |
| Random subdomain in flyout  | Auto-generated prefix in "Add responder"     | `await flyout.getByLabel('Subdomain prefix').clear()` before screenshot |
| ±1 diffs at icon/text edges | Sub-pixel anti-aliasing between browser runs | Handled by sticky-pixel stabilization (automatic)                       |
| Thin line diffs at edges    | Scrollbar visibility                         | Hidden by stability CSS (`::-webkit-scrollbar`)                         |
| Monaco editor differences   | Cursor, minimap, decorations                 | Hidden by stability CSS                                                 |
| Clipped region shifts       | Tooltip/bounding box sub-pixel jitter        | Use `Math.floor`/`Math.ceil` + generous padding                         |
| Animation artifacts         | CSS transitions captured in screenshot       | `addStyleTag` after `goto()` disables transitions before screenshots    |

**Important: Do NOT use `addInitScript` to inject stability CSS.** Injecting
`transition-duration: 0s` before the React app renders prevents `transitionend` events from
firing during EUI component initialization, causing the page to never finish loading. The CSS
must be injected AFTER navigation via `addStyleTag` so initial transitions complete normally.

### Test structure for docs screenshots

Each test follows a consistent step-based pattern:

1. **Step 1**: Navigate to the relevant page, highlight the primary action button (e.g.
   "Create responder", "Track page"), and take a screenshot of the empty/initial state.
2. **Create entity** - either via the UI form (for simple fields like Name, Path, Body
   textarea) or via API (for complex inputs like Monaco editor scripts). When using the
   API, reload the page afterward and open the Edit flyout to screenshot the pre-filled
   form.
3. **Subsequent steps**: Show the created entity in the grid, expand rows, click action
   buttons, and screenshot each meaningful state.

Screenshot naming convention: `{section}_step{N}_{description}.png`, e.g.
`html_step2_form.png`, `detect_resources_step7_responders_created.png`.

### Monaco editor fields

The Monaco code editor (used for Script and Content extractor fields) **cannot be reliably
filled** via Playwright's `.fill()` or `.pressSequentially()` - it times out or produces
syntax errors. Instead:

1. Create the entity via `page.request.post()` API with the script in the request body.
2. Reload the page, find the row, click **Edit**.
3. Scroll to the script section with `flyout.getByText('...').scrollIntoViewIfNeeded()`.
4. Screenshot the pre-filled form.

### Form interaction patterns

- **Name / Path / Body textarea**: Use `locator.fill(value)`.
- **Body textarea scroll**: After filling, reset scroll with
  `bodyTextarea.evaluate((el) => (el.scrollTop = 0))` so the screenshot shows the top.
- **Headers combo box**: Remove the default header first with
  `flyout.getByRole('button', { name: /Remove Content-Type/ }).click()`, then fill and
  press Enter on the combo box.
- **Combo boxes with substring labels**: Use `{ exact: true }` when one label is a prefix
  of another (e.g. "Key usage" vs "Extended key usage", "Export passphrase" vs "Repeat
  export passphrase").
- **Flyout close**: After screenshotting a form, close with
  `flyout.getByRole('button', { name: 'Close' }).click()` and assert
  `expect(flyout).not.toBeVisible()`.
- **Toast dismissal**: Call `dismissAllToasts(page)` after any save operation that triggers
  a success toast, before taking the next screenshot.
- **EUI actions column**: When a grid row has a collapsed actions menu, click
  `row.getByRole('button', { name: 'All actions, row' })` first, then select the action
  from the context menu scoped to the dialog/popover.
- **Responder URL links**: Locate the URL link in a responder grid row with
  `row.getByRole('link', { name: /\.webhooks\./ })`. The normalization in
  `waitForStableUiBeforeScreenshot` rewrites the visible text to `docs.webhooks.*` before
  screenshots, but the `href` attribute retains the real URL. Always read `href` **after**
  any screenshot that precedes the navigation:
  ```typescript
  await page.screenshot({ … });   // normalization runs here (text only, href untouched)
  const url = await responderLink.getAttribute('href');  // real URL, safe to navigate to
  await goto(newPage, url!);
  ```

### MDX documentation pattern

Each guide section in the `.mdx` files uses the `<Steps>` component with `<CodeBlock>` for
configuration tables. The pattern is:

```jsx
import Steps from '@site/src/components/Steps';
import CodeBlock from '@theme/CodeBlock';

<Steps steps={[
    {
        img: '../../img/docs/guides/<topic>/<screenshot>.png',
        caption: <>Navigate to ... and click <b>Action</b>.</>,
        alt: 'Description for accessibility.',
    },
    {
        img: '../../img/docs/guides/<topic>/<screenshot>.png',
        caption: <>Fill in the form and click <b>Save</b>.<br/><br/>
            <table className="su-table">
                <tbody>
                <tr><td><b>Name</b></td><td><CodeBlock>value</CodeBlock></td></tr>
                <tr><td><b>Body</b></td><td><CodeBlock language="html">{`<html>...</html>`}</CodeBlock></td></tr>
                </tbody>
            </table></>,
        alt: 'Fill in the form.',
    },
]} />
```

Key rules for MDX:
- Image paths in `<Steps>` use **relative paths** from the `.mdx` file to the `img/`
  directory (e.g. `../../img/docs/guides/...`).
- Inline markdown images (`![alt](/img/...)`) use **absolute paths** from the `static/`
  directory - Docusaurus resolves them differently.
- Escape template literal backticks and `${}` expressions inside `<CodeBlock>` JSX strings
  (e.g. `` \`...\` ``, `\${...}`).
- Do **not** nest JSX components like `<CodeBlock>` inside markdown numbered lists - the
  MDX parser cannot handle it. Use `<Steps>` or bold-text step numbers instead.

---

## Adding a new HTTP route

### Overview

Every public HTTP endpoint follows a layered pattern: an **actix handler** annotated with
**utoipa** OpenAPI metadata, backed by **typed request/response models** with `ToSchema`
derives, registered in the **server** and the **OpenAPI spec**, and covered by **sync-guard
tests** that keep schema examples honest.

This section walks through every file that must be touched.

### 1. Create or update the response model

Models live in their feature directory (e.g. `src/utils/certificates/private_keys/private_key.rs`).
Add `ToSchema` to the derive list and annotate any fields that use custom serde serializers:

```rust
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PrivateKey {
    pub id: Uuid,
    pub name: String,
    // OffsetDateTime serialises as a unix timestamp via `time::serde::timestamp`,
    // but utoipa cannot infer that — override with `value_type`.
    #[serde(with = "time::serde::timestamp")]
    #[schema(value_type = i64)]
    pub created_at: OffsetDateTime,
}
```

Key rules:
- `#[schema(value_type = i64)]` is required on every `OffsetDateTime` field serialized with
  `time::serde::timestamp`. Without it the OpenAPI schema emits an object instead of an integer.
- `#[serde(rename_all = "camelCase")]` is the project-wide convention for JSON field names.
- Fields hidden from the API (internal-only) use `#[serde(skip)]` or
  `#[serde(skip_serializing_if = "...")]`.
- If a handler returns a wrapper around multiple types, create a dedicated response struct
  in the handler file (see `CertificateTemplateGetResponse` in `certificate_templates.rs`).

### 2. Create request body params types (if needed)

Params types live in `src/utils/{feature}/api_ext/{name}_{verb}_params.rs`. Each file
defines one struct:

```rust
#[derive(Deserialize, Debug, Clone, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({"keyName": "my-key", "alg": {"keyType": "ed25519"}, "tagIds": []}))]
pub struct PrivateKeysCreateParams {
    pub key_name: String,
    pub alg: PrivateKeyAlgorithm,
    pub passphrase: Option<String>,
    #[serde(default)]
    pub tag_ids: Vec<Uuid>,
}
```

Key rules:
- **Every params type must have a `#[schema(example = json!(...))]` attribute.** The example
  must be realistic (non-empty names, valid enum variants) and must actually deserialize into
  the struct - this is enforced by sync-guard tests (see step 6).
- Re-export the type from the feature's `{feature}.rs` so it's importable as
  `crate::utils::{feature}::ParamsType`.

### 3. Create the handler file

Handlers live in `src/server/handlers/{feature}.rs`. A handler file contains:

#### Path parameter struct (if the route has path variables)

```rust
#[derive(serde::Deserialize, IntoParams)]
pub struct KeyIdPath {
    pub key_id: Uuid,
}
```

#### Handler functions

Each function is annotated with both a utoipa doc macro and an actix HTTP-method macro:

```rust
/// Returns a list of all private keys for the authenticated user.
#[utoipa::path(
    tags = ["certificates"],
    responses(
        (status = 200, description = "Successfully retrieved private keys.", body = [PrivateKey])
    )
)]
#[get("/api/certificates/private_keys")]
pub async fn private_keys_list(
    state: web::Data<AppState>,
    user: User,
) -> Result<HttpResponse, Error> {
    let keys = state.api.certificates().get_private_keys(user.id).await?;
    Ok(HttpResponse::Ok().json(keys))
}
```

Conventions:
- **Tags** group endpoints in the generated docs (e.g. `"certificates"`, `"webhooks"`).
- **Route paths** use `/api/{feature}/{resource}` with `{param}` placeholders.
- **Action endpoints** (non-CRUD operations) use a `_` prefix: `/{id}/_export`, `/_generate`.
- **HTTP method macros** (`#[get]`, `#[post]`, `#[put]`, `#[delete]`) take the full route
  path — there are no scopes/prefixes.
- **Extractors** appear as function parameters: `web::Data<AppState>`, `User` (custom auth
  extractor), `web::Path<ParamStruct>`, `web::Json<BodyType>`.
- **Response codes**: `Ok` for reads/updates, `Created` for creates, `NoContent` for deletes.
- **`params(StructName)`** in the utoipa macro is required when the route has path variables.
- **`request_body = Type`** is required when the handler accepts a JSON body.

#### Sync-guard tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::schema_example;

    #[test]
    fn private_keys_create_params_example_is_valid() {
        let example: PrivateKeysCreateParams =
            serde_json::from_value(schema_example::<PrivateKeysCreateParams>()).unwrap();
        assert!(!example.key_name.is_empty());
    }
}
```

- Write one test per params type that has a `#[schema(example = ...)]`.
- `schema_example::<T>()` (defined in `src/main.rs`) extracts the example JSON from the
  utoipa-generated schema and returns a `serde_json::Value`.
- The test deserializes it into the real type - if the example drifts out of sync with the
  struct definition, the test fails at compile time or at runtime.
- Add at least one semantic assertion (e.g., name is non-empty) to keep examples realistic.

### 4. Declare the handler module (`src/server/handlers.rs`)

Add the module declaration:

```rust
pub mod private_keys;
```

Then register all handler paths and schema types in the `#[openapi(...)]` macro:

```rust
#[derive(OpenApi)]
#[openapi(
    paths(
        // ... existing paths ...
        private_keys::private_keys_list,
        private_keys::private_keys_get,
        private_keys::private_keys_create,
        private_keys::private_keys_update,
        private_keys::private_keys_delete,
        private_keys::private_keys_export,
    ),
    components(schemas(
        // ... existing schemas ...
        PrivateKey,
        PrivateKeysCreateParams,
        PrivateKeysUpdateParams,
        PrivateKeysExportParams,
    ))
)]
pub struct SecutilsOpenApi;
```

**Both lists must be updated.** If a handler references a type in `request_body` or `body`
that is not listed in `components(schemas(...))`, it will silently generate an unresolved
`$ref` in the OpenAPI spec.

### 5. Register handlers in the server (`src/server.rs`)

Add a `.service()` call for each handler function:

```rust
.service(handlers::private_keys::private_keys_list)
.service(handlers::private_keys::private_keys_get)
.service(handlers::private_keys::private_keys_create)
.service(handlers::private_keys::private_keys_update)
.service(handlers::private_keys::private_keys_delete)
.service(handlers::private_keys::private_keys_export)
```

Group them by feature, following the order of existing registrations.

### 6. Update OpenAPI snapshot tests (`src/server/handlers.rs`)

The file contains inline `insta::assert_json_snapshot!` tests that pin the OpenAPI output.
After adding new routes, at minimum these snapshots need updating:

- **`openapi_spec_has_all_paths`** - add the new route paths (sorted alphabetically).
- **`openapi_spec_has_all_schemas`** - add the new schema names (sorted alphabetically).

Optionally add dedicated snapshot tests for the new endpoints:

```rust
#[test]
fn openapi_spec_private_keys_crud_operations() {
    let spec = spec();
    let path = &spec["paths"]["/api/certificates/private_keys"];
    assert_json_snapshot!(path, @r###"..."###);
}
```

Run `cargo test` and use `cargo insta review` to accept snapshot updates.

### 7. Update the Web UI

API endpoint URLs are defined in the UI layer (TypeScript/React). When migrating from the
generic dispatcher or adding new routes:

- Search for the old URL pattern (e.g. `/api/utils/certificates/private_keys`) across
  `components/secutils-webui/src/` and update to the new pattern.
- Action endpoints change from `/{id}/action` to `/{id}/_action` (underscore prefix).

### 8. Update E2E tests

E2E tests intercept API calls. Search for the old URL pattern across `e2e/tests/` and
`e2e/docs/` and update:

- `page.route()` interceptors
- `page.request.post/get/put/delete()` calls
- URL assertions

### 9. Update the HTTP dev file

API test files live in `dev/api/`. Update or create a `.http` file for the new endpoints
(e.g. `dev/api/utils/certificates_private_keys.http`).

### 10. Update API documentation

When introducing a **new API group** (i.e. a new `tags` value in the `#[openapi]` macro):

1. **OpenAPI tag description** — add an entry to the `tags(...)` list in the `#[openapi]`
   macro in `src/server/handlers.rs`:

   ```rust
   tags(
       // ... existing tags ...
       (name = "new_feature", description = "Short description of the API group."),
   )
   ```

2. **Docs API reference page** — add a row to the "Available API groups" table in
   `components/secutils-docs/docs/project/api.md`:

   ```markdown
   | `new_feature` | `/api/new_feature/...` | Short description |
   ```

3. **Rebuild docs** — run `npm run build` in `components/secutils-docs/` and verify the new
   entry appears in both the rendered page and `build/llms.txt` (used by LLM crawlers).

When modifying an **existing** API group (renaming routes, changing base paths), update the
same two locations to keep them in sync.

### Verification checklist

After all changes, run:

```bash
# All Rust tests (includes sync-guard tests and snapshot tests)
cargo test

# Lint check
cargo clippy

# Web UI build (catches broken imports / URL references)
npm run build --prefix components/secutils-webui
```

All three must pass cleanly before the change is considered complete.

### Quick reference: file locations

```
src/server/handlers.rs                           # OpenAPI macro (paths + schemas + tags), snapshot tests
src/server/handlers/{feature}.rs                 # Handler functions, path params, sync-guard tests
src/server.rs                                    # .service() registration
src/utils/{feature}/.../model.rs                 # Response model with ToSchema
src/utils/{feature}/api_ext/*_params.rs          # Request body types with ToSchema + example
src/main.rs                                      # schema_example::<T>() test helper
components/secutils-webui/src/                   # Web UI API calls
components/secutils-docs/docs/project/api.md     # API reference page (linked from llms.txt)
e2e/tests/                                       # E2E tests
dev/api/                                         # .http dev files
```

---

## JS Runtime Performance Harness (`benches/js-runtime-perf/`)

### Overview

The perf harness measures latency, throughput, and peak RSS delta of the embedded Deno/V8
runtime used for responder scripts and other user JavaScript. It lives in the workspace at
`benches/js-runtime-perf/` and exposes a single `js-runtime-perf` binary that links against
the real `secutils::js_runtime::JsRuntime`, so changes to the runtime (pooling, snapshots,
shared `reqwest::Client`, etc.) show up directly in the recorded numbers.

The harness is **advisory / warn-only**. CI records a new history entry on every push to
`main` and prints a table with per-metric deltas, but it never fails a build on regressions.
Thresholds in `.perf/config.json` only control when warnings are emitted.

### Scenario catalogue

All scenarios use `JsRuntimeConfig { max_heap_size: 10 MiB, max_user_script_execution_time:
10s }` and a realistic payload size so the numbers track production behaviour.

| Scenario                    | What it measures                                                                                                 |
|-----------------------------|------------------------------------------------------------------------------------------------------------------|
| `cold_start_trivial`        | Full per-call cost: `spawn_blocking` + fresh `CurrentThread` runtime + fresh V8 isolate + watchdog, trivial JS.  |
| `steady_state_trivial`      | Serial executions of a trivial script. Exposes per-call overhead without startup amortisation.                   |
| `responder_like`            | Wrapped responder script (`wrap_script_with_body_conversion`) with a realistic `{body, headers, method}` input.  |
| `proxy_request`             | `op_proxy_request` against an in-process `httpmock` server on 127.0.0.1 - rebuilds `reqwest::Client` per call.   |
| `concurrent_responders_8x`  | `tokio::spawn` burst of `N` trivial scripts; latency is per-task wall clock.                                     |

### Running locally

```bash
# Full run + comparison table + history append (default 500 iterations, 50 warmup, 8-way concurrency)
make perf ANALYZE=1

# Run only, no history touch (useful when iterating locally and discarding results)
make perf

# Re-analyze an existing /tmp/perf.json (e.g. downloaded from CI) without rerunning
make perf-analyze

# Smoke test (fast)
make perf ANALYZE=1 PERF_ITERATIONS=20 PERF_WARMUP=5

# Single scenario
make perf ANALYZE=1 PERF_SCENARIOS=responder_like

# Custom output path (e.g. to compare two branches)
make perf PERF_OUTPUT=/tmp/perf-baseline.json

# View HTML report (opens scripts/perf-report.html, then load .perf/history.jsonl)
make perf-report
```

`make perf` produces `/tmp/perf.json` and prints a one-line summary per scenario. When
`ANALYZE=1` is set it then invokes `scripts/analyze-perf.ts`, which compares the fresh
report to the last entry in `.perf/history.jsonl`, prints a table with Δp50/Δp99/Δops/Δrss
columns, and appends to history **only when at least one tracked metric moved by more
than 0.1 %** (see "History append gating" below). `make perf-analyze` is the same
analyze-only tail, exposed separately for re-analyzing a file without rerunning the
harness.

### Interpreting the output

The printed table uses the last recorded history entry as the baseline:

```
Scenario                             p50       p99    throughput       rss      Δp50      Δp99      Δops      Δrss
cold_start_trivial                2.85ms    2.89ms       361.9/s     928KB     -3.1%     -2.0%     +2.4%      0.0%
```

- **Δp50 / Δp99**: percentage change in latency vs the previous run. Warnings fire when
  these exceed the thresholds in `.perf/config.json` (`p50`, `p99`).
- **Δops**: percentage change in throughput. Warnings fire on a _decrease_ below
  `-thresholds.throughput` (i.e. getting slower).
- **Δrss**: percentage change in peak RSS delta. Warnings fire above
  `thresholds.peakRssDeltaKb`.

A first run prints "First run recorded – no comparison available." and establishes the
baseline.

### History append gating

`scripts/analyze-perf.ts` does not append unconditionally. It diffs the fresh report
against the last entry in `.perf/history.jsonl` across a whitelist of tracked metrics
(`p50_us`, `p90_us`, `p99_us`, `max_us`, `throughput_ops_per_sec`, `peak_rss_delta_kb`).
If every tracked metric on every scenario is within ±0.1 % of the previous entry, the
file is left untouched and the CLI prints `All tracked metrics within ±0.1% of the
previous run; history not updated.` When something moves, the append happens and the
output names the scenario/metric that tripped the threshold.

This matters for the CI commit step: because `history.jsonl` is modified only on
material movement, the `git diff --cached --quiet || git commit` check becomes an
effective "commit only if something changed" — pushes with steady-state numbers no
longer produce noisy chore commits on `main`.

The threshold is hard-coded at `HISTORY_APPEND_THRESHOLD_PCT = 0.1` in
`scripts/analyze-perf.ts`. Adjust there if it proves too tight or too loose.
Scenario additions/removals are treated as unconditionally material (always appended).
Structural zero-valued metrics (e.g. `peak_rss_delta_kb = 0`) are handled explicitly —
`0 → 0` is unchanged, `0 → anything` or `anything → 0` triggers an append.

### CI contract

- `.github/workflows/ci.yml` has a `ci-perf` job that runs on every push to `main`.
- It builds the harness in release mode, runs `make perf ANALYZE=1` (which produces
  the report, prints the delta table, and appends to history only on material
  movement), uploads `/tmp/perf.json` as an artefact, and commits the updated
  `.perf/history.jsonl` back to `main` with `[skip ci]` in the commit message.
- The commit step is a no-op when nothing moved — `history.jsonl` is unmodified, so
  `git diff --cached --quiet` is true.
- The job **never fails on regressions**. Warnings are visible in the job log; acting on
  them is a human decision.

### File locations

```
benches/js-runtime-perf/Cargo.toml               # Workspace member, depends on `secutils`
benches/js-runtime-perf/src/main.rs              # CLI driver
benches/js-runtime-perf/src/measure.rs           # hdrhistogram recorder, peak RSS probe
benches/js-runtime-perf/src/report.rs            # JSON output shape (camelCase top-level)
benches/js-runtime-perf/src/scenarios/*.rs       # One scenario per file
benches/js-runtime-perf/scripts/*.js             # JS fixtures loaded via `include_str!`
src/lib.rs                                       # Minimal library target exposing `js_runtime`
.perf/config.json                                # Scenario list + warning thresholds
.perf/history.jsonl                              # Append-only history (one JSON per run)
scripts/analyze-perf.ts                          # Node 22 analyzer (reads /tmp/perf.json)
scripts/perf-report.html                         # Standalone HTML viewer for history.jsonl
```

### Tuning

- To relax or tighten warnings, edit `.perf/config.json`. Values are percentages.
- To add a scenario: create a module under `benches/js-runtime-perf/src/scenarios/`,
  register it in `scenarios.rs` (both the `ALL` slice and the `run` dispatcher), and add
  its name to `.perf/config.json`.
- Benchmark results are platform-sensitive. History entries include `env.os`, `env.arch`,
  and `env.cpuModel` for this reason; absolute numbers from a laptop are not directly
  comparable to those from a CI runner.
