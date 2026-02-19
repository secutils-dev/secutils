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

### Locator strategy

Choose locators in this order of preference. Prefer semantic, user-visible locators that
mirror how a real user perceives the page.

1. **`getByRole`** — first choice for buttons, headings, links, and other ARIA-role elements.
   Use `name` to disambiguate and `exact: true` when the label is a substring of another
   element's label.

   ```typescript
   page.getByRole('button', { name: 'Sign up', exact: true });
   page.getByRole('heading', { name: 'Welcome', level: 2 });
   ```

2. **`getByPlaceholder`** — preferred for form inputs that carry placeholder text. Use
   `exact: true` when a shorter placeholder is a prefix of another (e.g. "Password" vs
   "Repeat password").

   ```typescript
   page.getByPlaceholder('Email');
   page.getByPlaceholder('Password', { exact: true });
   ```

3. **`getByText`** — fallback for elements that have visible text but no clear ARIA role or
   placeholder (e.g. menu items, labels).

   ```typescript
   page.getByText('Sign out');
   ```

4. **Raw `locator` (CSS/XPath)** — last resort for cases where no semantic locator is
   practical, such as checking that *any* form control is present without caring about exact
   text.

   ```typescript
   page.locator('input[name="password"], input[name="identifier"], form');
   ```

### Timeouts

The Docker-based stack (Kratos auth + API + Web UI) can be slow to respond, especially on
the first load. Use explicit timeouts on visibility and URL assertions:

- **`toBeVisible({ timeout: 15000 })`** — for elements that depend on the page fully
  rendering or an auth redirect completing.
- **`toHaveURL(pattern, { timeout: 30000 })`** — for navigations that involve server-side
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
- Use **`type` imports** (`import type { … }`) where possible — enforced by
  `consistent-type-imports`.
- Import order: builtins, externals, internals, then parent/sibling/index — alphabetized,
  separated by blank lines.
- No unused variables or expressions.
