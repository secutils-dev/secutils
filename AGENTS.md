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

### Shared helpers (`e2e/docs/helpers.ts`)

All docs tests import from `helpers.ts`. Key exports:

| Helper                                      | Purpose                                                                                                                                 |
|---------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------|
| `ensureUserAndLogin(request, page)`         | Remove existing user, register a fresh one, and log in.                                                                                 |
| `goto(page, url)`                           | Navigate and inject a stylesheet that disables all CSS animations/transitions for deterministic screenshots.                            |
| `highlightOn(locator)`                      | Add a red dashed outline around an element for visual emphasis.                                                                         |
| `highlightOff(locator)`                     | Remove the highlight outline.                                                                                                           |
| `dismissAllToasts(page)`                    | Dismiss every visible toast notification (iterate all, not just one).                                                                   |
| `fixResponderRequestFields(page)`           | Intercept responder request history API and pin `createdAt`/`clientAddress` to fixed values.                                            |
| `fixCertificateTemplateValidityDates(page)` | Pin `notValidBefore`/`notValidAfter` to fixed dates while preserving their duration.                                                    |
| `fixTrackerResourceRevisions(page)`         | Stabilize tracker revision history: strip URL query strings, normalize webhook subdomains, compute deterministic sizes, fix timestamps. |

### Screenshot stability

Screenshots must be **deterministic** - running the tests twice should produce identical
images. Any dynamic value that leaks into a screenshot causes unnecessary diffs.

Common sources of instability and how to fix them:

- **Timestamps / dates**: Intercept the API response with `page.route()` and replace
  dynamic timestamps with a fixed epoch value (e.g. `1740000000`).
- **Client addresses**: Pin to a fixed value like `172.18.0.1:12345`.
- **CSP nonces**: Intercept responses and replace rotating nonces with a fixed value
  (e.g. `nonce-m0ck`).
- **URL query strings**: Strip random cache-buster parameters from resource URLs.
- **Webhook subdomains**: Normalize user-specific subdomains to a fixed value
  (e.g. `preview.webhooks.secutils.dev`).
- **Cryptographic output** (JWK values, key exports): Replace dynamic fields via
  `element.evaluate()` after the UI renders them.
- **CSS animations**: Already handled by `goto()` which injects
  `animation-duration: 0s !important; transition-duration: 0s !important`.

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
