# Application and standalone E2E

Run `make` commands from the Secutils root. Run browser installation from `e2e/`.
For standalone tool tests in `e2e/tools/`, read `dev/tools/AGENTS.md`.
For docs tests, also read [docs screenshots](docs-screenshots.md).

## Run the relevant stack

Application tests in `e2e/tests/` use the full Compose stack at `http://localhost:7171`.

```bash
make e2e-up
make e2e-test ARGS="tests/registration.spec.ts"
```

Use `make help` for headed/UI runs, reports, logs, and teardown.
Standalone tests use `e2e/playwright.standalone.config.ts` and need no application stack:

```bash
# From e2e/:
npx playwright install chromium
# From the root:
make e2e-standalone-test
```

Read `e2e/standalone/` for the current test inventory.
The suite covers codegen transformation, webhook encryption wire parity, and responder script execution.
CI runs it before the full stack.

### Serve the changed code

`make docker-*` builds separately tagged images that the E2E stack does not consume.
Rebuild the Compose service before assertions against changed source:

```bash
make e2e-up BUILD=1 ONLY=secutils_webui
```

`ONLY=` and `SKIP=` narrow builds, including expensive Rust and Camoufox images.
If behavior disagrees with source, inspect the served container asset before debugging the test.

## Write tests

Use existing specs as examples. Keep one spec per feature area.
Group related tests with `test.describe`.
For tests that create server state, clean it in `beforeEach` through the internal API.
Keep credentials and operator tokens as module-level constants.
Use `page.request` to assert backend state when UI assertions are insufficient.
Use `e2e` lint/format configuration as the code-style source of truth.

### Locators and EUI

Prefer `getByRole`, then `getByPlaceholder`, then `getByText`.
Use `exact: true` for labels that overlap.
For EUI internals without semantic locators, use `@elastic/eui-test-helpers` where supported.
Use raw CSS/XPath only where those locators and helpers cannot identify the element.

EUI helpers need `testIdAttribute: 'data-test-subj'` and a matching component `data-test-subj`.
A missing helper root can silently do nothing. Assert that the root is attached in shared helpers.
For toast `closeAll`, wrap the operation in `toPass` to include toasts that arrive during dismissal.

For `announceOnMount` callouts, assert `toContainText` on the enclosing container.
EUI temporarily duplicates callout text in `role="status"`.
A text locator can match both copies and cause an immediate strict-mode failure.

### Timeouts

Use `toBeVisible({ timeout: 15000 })` for initial rendering or auth redirects.
Use `toHaveURL(pattern, { timeout: 30000 })` for server-driven navigation.
Use ordinary assertions after the relevant page element is already awaited.

## Diagnose flakes

```bash
make e2e-test-loop ARGS="tests/registration.spec.ts" RUNS=20
```

Read `/tmp/e2e-loop-results/run-N.log` and `artifacts-run-N/` for failed runs.
The loop summary reports pass/fail counts. Require zero failed runs to establish a stable test.
If browser execution is blocked, ask the user to run the same command and share its results.
Report that verification as pending until the results are available.

Completion requires a passing relevant suite against the changed code.
For coordinated Playwright upgrades, use the full-suite gate in [dependency upgrades](dependency-upgrades.md).
