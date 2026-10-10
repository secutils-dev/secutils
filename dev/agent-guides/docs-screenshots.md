# Docs screenshots and screenshot-based MDX

Read [E2E](e2e.md) for stack rebuilds, locators, cleanup, and timeouts.
Commands run from the Secutils root and require the full E2E stack.
Specs live in `e2e/docs/`. Images go to `components/secutils-docs/static/img/docs/guides/<topic>/`.

## Generate screenshots

```bash
make docs-screenshots ARGS="docs/csp.spec.ts"
```

Use `e2e/helpers.ts` for navigation, highlighting, toast dismissal, and deterministic data.
Read its exports for the current helper inventory.
Use `goto()` for every screenshot page so navigation installs the screenshot patch.
Inject stability CSS after navigation with `addStyleTag`.
**Keep stability CSS out of `addInitScript`.** Early injection suppresses EUI initialization `transitionend` events and can prevent loading.

The patch waits for layout, fonts, and icons. It hides cursors, scrollbars, transitions, and other dynamic editor decorations.
It normalizes visible webhook subdomains but preserves link `href` attributes.
Sticky-pixel stabilization restores reference bytes when all RGBA channels differ by at most `MAX_CHANNEL_DIFF` (±1).
Keep that threshold fixed. Larger differences require diagnosis.

### Stabilize dynamic data

Install route interceptors before the request. Use `route.fetch()` and `route.fulfill({ response, json })` to preserve real responses.
For handlers that receive different response shapes, guard arrays before iteration.
Paginated entity responses use `{ items, total }`. `pinEntityTimestamps` supports that wrapper.

| Dynamic field                     | Stabilization                                                                                                                                |
|-----------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------|
| Entity timestamps                 | `fixEntityTimestamps` or `pinEntityTimestamps` with `FIXED_ENTITY_TIMESTAMP`. An old date produces absolute dates instead of relative times. |
| Responder history date/address    | `fixResponderRequestFields`                                                                                                                  |
| Certificate validity              | `fixCertificateTemplateValidityDates`, preserving the validity duration                                                                      |
| Tracker revision URLs/sizes/dates | `fixTrackerResourceRevisions`                                                                                                                |
| Tracker execution phase times     | `fixTrackerExecutionLogs`                                                                                                                    |
| Tracker health dots               | `fixTrackerHealthDots`                                                                                                                       |
| Home summary                      | Intercept `/api/ui/home/summary` and pin `recentItems` timestamps                                                                            |
| CSP nonces                        | Replace rotating values, for example with `nonce-m0ck`                                                                                       |
| Other client addresses            | Use a fixed value such as `172.18.0.1:12345`                                                                                                 |
| Random URL parameters             | Strip cache-buster query strings                                                                                                             |
| Cryptographic exports/JWKs        | Replace dynamic rendered fields through `element.evaluate()`                                                                                 |
| Add-responder prefix input        | `await flyout.getByLabel('Subdomain prefix').clear()`                                                                                        |

Read a responder link's real `href` after any preceding screenshot, then navigate with that value.
The screenshot patch changes the visible text to `docs.webhooks.*`.

### Screenshot sequence and form interactions

1. Navigate to the initial page and highlight its primary action.
2. Capture the initial state.
3. Create the entity and capture its filled form.
4. Capture the saved grid, expanded details, and subsequent meaningful actions.

Use `{section}_step{N}_{description}.png` names.
For simple fields, use `fill()`.
For Monaco scripts or extractors, create the entity through `page.request.post()`, then reload and open Edit.
Scroll to the prefilled script section before capture. Direct Monaco typing is unreliable.
After body textarea input, reset `scrollTop` to zero.
For header combo boxes, remove the default `Content-Type` before a replacement.
For overlapping combo-box labels, use `exact: true`.
After save, call `dismissAllToasts(page)` before the next screenshot.
After flyout close, await `not.toBeVisible()`.
For collapsed row actions, open `All actions, row` before the scoped menu action.

For clipped tooltip screenshots, use integer bounds and generous padding:

```typescript
const PAD = 16;
const x = Math.floor(Math.min(sectionBox.x, tooltipBox.x)) - PAD;
const y = Math.floor(Math.min(sectionBox.y, tooltipBox.y)) - PAD;
const right = Math.ceil(Math.max(sectionBox.x + sectionBox.width, tooltipBox.x + tooltipBox.width)) + PAD;
const bottom = Math.ceil(Math.max(sectionBox.y + sectionBox.height, tooltipBox.y + tooltipBox.height)) + PAD;
await page.screenshot({ path, clip: { x, y, width: right - x, height: bottom - y } });
```

## Diagnose drift

```bash
make docs-screenshots-diff ARGS="docs/csp.spec.ts"
make docs-screenshots-analyze
```

Read `/tmp/screenshot-diff/report.txt` and `analysis-report.json`.
Images and logs live beside them in `run-a/`, `run-b/`, `diffs/`, and `analysis/`.

1. Classify each changed file as byte-only or pixel drift.
2. For pixel drift, inspect the annotated region and crop the source images to identify the element.
3. Fix its dynamic data, bounding box, or missing stabilization.
4. For byte-only drift, inspect PNG encoding/chunks and the existing screenshot patch.
5. Run `make docs-screenshots-loop ARGS="docs/csp.spec.ts" RUNS=10`.
6. Repeat the two-run comparison to establish byte stability.

Completion requires passing runs and no unexplained screenshot changes.
New screenshots need an initial baseline before the byte comparison.
Treat jitter beyond ±1 as a diagnosed residual, rather than increasing the sticky threshold.
`highlightOn` already settles the UI to avoid highlights on nodes that React replaces.

## MDX guide content

Use the existing `<Steps>` component for screenshot sequences.
Use `<CodeBlock>` in a caption's `table.su-table` for configuration values.
Read an existing guide for the JSX structure.

- Use relative `../../img/docs/guides/...` paths in `<Steps>` image props.
- Use absolute `/img/...` paths for inline Markdown images.
- Escape backticks and `${}` in JSX template literals inside `<CodeBlock>`.
- Keep JSX components outside Markdown numbered lists. Use `<Steps>` or bold-text step numbers.
- Keep screenshot order, captions, alt text, and configuration values aligned with the test sequence.

After MDX changes, run the docs build and inspect the rendered guide.
For output-pipeline changes, also read [docs output](docs-output.md).
