# AGENTS.md

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
```

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
`web_scraping.spec.ts`). Screenshots are saved directly into
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
| `pinEntityTimestamps(json)`                 | Replace `createdAt`/`updatedAt` with `FIXED_ENTITY_TIMESTAMP` in a JSON value.                                                          |
| `fixEntityTimestamps(page, pattern)`        | Set up a route handler that pins timestamps in GET JSON responses matching `pattern`.                                                   |
| `fixResponderRequestFields(page)`           | Intercept responder request history API and pin `createdAt`/`clientAddress` to fixed values.                                            |
| `fixCertificateTemplateValidityDates(page)` | Pin `notValidBefore`/`notValidAfter` to fixed dates while preserving their duration.                                                    |
| `fixTrackerResourceRevisions(page)`         | Stabilize tracker revision history: strip URL query strings, normalize webhook subdomains, compute deterministic sizes, fix timestamps. |

### Screenshot stability

Screenshots must be **byte-identical** across runs. The stability system has multiple layers
that work together automatically when using `goto()`:

#### Automatic stabilization (handled by `goto()` and `patchPageScreenshot`)

These apply to every screenshot without any test-level code:

1. **CSS injection** — `goto()` injects a `<style>` tag after navigation that:
   - Disables all CSS animations and transitions (`animation-duration: 0s; transition-duration: 0s`).
   - Forces greyscale anti-aliasing (`-webkit-font-smoothing: antialiased; text-rendering: geometricPrecision`) — reduces font rendering variance from ±8 to ±1.
   - Forces icon buttons and toggle switches into GPU compositing layers (`.euiButtonIcon, .euiSwitch__body { will-change: transform }`) — reduces SVG/toggle rendering variance from ±24 to ±1.
   - Hides Monaco editor non-deterministic elements (cursor layer, minimap, decorations overview ruler, scroll decoration).
   - Hides the system text caret (`caret-color: transparent`).
   - Hides scrollbars (`::-webkit-scrollbar { width: 0; height: 0 }`).

2. **Pre-screenshot stabilization** — `waitForStableUiBeforeScreenshot()` runs before every
   `page.screenshot()` call and:
   - Waits for `domcontentloaded` and `networkidle` (with 5 s timeout).
   - Waits for all EUI icons to finish loading (`.euiIcon[data-is-loading="true"]`).
   - Waits for all web fonts to reach `loaded` status (`document.fonts.status`).
   - Normalizes webhook URLs in the DOM — replaces user-specific UUIDs in
     `/api/webhooks/u/<uuid>/` with `/api/webhooks/u/preview/` in links, input values,
     code blocks, and data grid popovers.
   - Waits three animation frames for layout/paint/composite to settle.

3. **Sticky-pixel screenshot stabilization** — `stabilizeScreenshot()` runs after
   every `page.screenshot()`.  Before the screenshot is taken, the existing file on disk
   (if any) is saved as a byte buffer.  After capturing, both the reference and new PNGs
   are decoded to raw RGBA pixels with `pngjs` (`PNG.sync.read`).  If every channel value
   in the new image is within ±1 of the reference (`MAX_CHANNEL_DIFF`), the image has
   not meaningfully changed — the original reference bytes are written back verbatim,
   producing zero diff.  This absorbs non-deterministic sub-pixel anti-aliasing jitter
   from Chromium's GPU compositor between browser sessions.  When any pixel genuinely
   differs (channel diff > 1) or the dimensions changed, the new Playwright file is kept
   as-is and becomes the new baseline for future runs.

#### Test-level stabilization (must be added per test/describe)

Each source of dynamic data needs explicit stabilization in the test code:

- **Timestamps / dates**: Intercept the API response with `page.route()` and replace
  dynamic timestamps with `FIXED_ENTITY_TIMESTAMP` (epoch `1740000000`, renders as
  "February 19, 2025" — deliberately >3 days old so the UI shows an absolute date
  instead of a relative string like "a few seconds ago").
- **Client addresses**: Pin to a fixed value like `172.18.0.1:12345`.
- **CSP nonces**: Intercept responses and replace rotating nonces with a fixed value
  (e.g. `nonce-m0ck`).
- **URL query strings**: Strip random cache-buster parameters from resource URLs.
- **Webhook subdomains**: Normalize user-specific subdomains to a fixed value
  (e.g. `preview.webhooks.secutils.dev`).
- **Cryptographic output** (JWK values, key exports): Replace dynamic fields via
  `element.evaluate()` after the UI renders them.
- **Home page summary**: Intercept `/api/ui/home/summary` and call `pinEntityTimestamps()`
  on `recentItems` to avoid relative time strings.

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
   - `Byte-identical` — no action needed.
   - `Byte-diff only (0 pixel diffs)` — DEFLATE compression non-determinism (should be
     resolved by `reEncodePngDeterministic`; if it re-appears, check for PNG chunk changes).
   - Files with pixel diffs > 0 — need investigation (see below).
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

| Symptom                     | Likely Cause                                  | Fix                                                              |
|-----------------------------|-----------------------------------------------|------------------------------------------------------------------|
| Byte-diff but no pixel diff | PNG DEFLATE non-determinism or ±1 AA jitter   | `stabilizeScreenshot()` (automatic — restores reference file)    |
| Text changes between runs   | Relative timestamps ("a few seconds ago")     | `fixEntityTimestamps()` or `pinEntityTimestamps()`               |
| URL segments differ         | User-specific webhook UUIDs                   | Automatic DOM normalization in `waitForStableUiBeforeScreenshot` |
| ±1 diffs at icon/text edges | Sub-pixel anti-aliasing between browser runs  | Handled by sticky-pixel stabilization (automatic)                |
| Thin line diffs at edges    | Scrollbar visibility                          | Hidden by stability CSS (`::-webkit-scrollbar`)                  |
| Monaco editor differences   | Cursor, minimap, decorations                  | Hidden by stability CSS                                          |
| Clipped region shifts       | Tooltip/bounding box sub-pixel jitter         | Use `Math.floor`/`Math.ceil` + generous padding                  |
| Animation artifacts         | CSS transitions captured in screenshot        | `addStyleTag` after `goto()` disables transitions before screenshots |

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
