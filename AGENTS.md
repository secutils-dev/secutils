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
   - Normalizes webhook URLs in the DOM - replaces user-specific UUIDs in
     `/api/webhooks/u/<uuid>/` with `/api/webhooks/u/preview/` in links, input values,
     code blocks, and data grid popovers.
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
- **Webhook subdomains**: Normalize user-specific subdomains to a fixed value
  (e.g. `preview.webhooks.secutils.dev`).
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

| Symptom                     | Likely Cause                                 | Fix                                                                  |
|-----------------------------|----------------------------------------------|----------------------------------------------------------------------|
| Byte-diff but no pixel diff | PNG DEFLATE non-determinism or ±1 AA jitter  | `stabilizeScreenshot()` (automatic - restores reference file)        |
| Text changes between runs   | Relative timestamps ("a few seconds ago")    | `fixEntityTimestamps()` or `pinEntityTimestamps()`                   |
| URL segments differ         | User-specific webhook UUIDs                  | Automatic DOM normalization in `waitForStableUiBeforeScreenshot`     |
| ±1 diffs at icon/text edges | Sub-pixel anti-aliasing between browser runs | Handled by sticky-pixel stabilization (automatic)                    |
| Thin line diffs at edges    | Scrollbar visibility                         | Hidden by stability CSS (`::-webkit-scrollbar`)                      |
| Monaco editor differences   | Cursor, minimap, decorations                 | Hidden by stability CSS                                              |
| Clipped region shifts       | Tooltip/bounding box sub-pixel jitter        | Use `Math.floor`/`Math.ceil` + generous padding                      |
| Animation artifacts         | CSS transitions captured in screenshot       | `addStyleTag` after `goto()` disables transitions before screenshots |

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
