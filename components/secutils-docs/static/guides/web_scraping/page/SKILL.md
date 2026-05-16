---
name: secutils-web-scraping-page
description: >-
  Create, list, update, debug, and run page trackers on Secutils.dev. A
  page tracker is a scheduled headless Playwright job that visits a target
  URL, executes a user-supplied extractor script in a Chromium browser,
  stores up to N historical revisions of the extracted value (text, JSON,
  HTML, or any payload up to 1 MB), and notifies the user when the value
  changes. Trigger when the user asks to "monitor a web page for changes",
  "track the top Hacker News post", "alert me when X on a website changes",
  detect deployment regressions, watch a competitor's marketing page, or
  monitor JavaScript and CSS resources on a deployed web app. Requires an
  authenticated Kratos session cookie or a Secutils API key.
---

# Secutils.dev: Web Scraping (Page Trackers)

A page tracker schedules a headless Playwright (Chromium) navigation to
a URL, runs a user-defined `execute(page, context)` function inside the
Retrack scraper sandbox, stores the returned value as the latest
revision, and optionally notifies the user when the value differs from
the previous revision. Trackers can also extract structured `script` and
`stylesheet` resource descriptors (URL, content hash, size) for
synthetic monitoring of deployed web apps.

Unlike API trackers (see `secutils-web-scraping-api`), page trackers
launch a real browser, so they handle SPA hydration, lazy-loaded assets,
and DOM interactions. The trade-off is higher latency per run and a
single 1 MB cap on the extracted body. Pages behind a CAPTCHA or web
application firewall are not supported.

Guide: <https://secutils.dev/docs/guides/web_scraping/page>.
Full reference: <https://secutils.dev/api-docs/openapi.json>.

## Endpoints (tag: `web_scraping`)

| Method   | Path                                                       | Purpose                                                                     |
|----------|------------------------------------------------------------|-----------------------------------------------------------------------------|
| `GET`    | `/api/web_scraping/page_trackers`                          | List all page trackers for the current user.                                |
| `POST`   | `/api/web_scraping/page_trackers`                          | Create a tracker (`PageTrackerCreateParams`).                               |
| `PUT`    | `/api/web_scraping/page_trackers/{tracker_id}`             | Replace a tracker's configuration.                                          |
| `DELETE` | `/api/web_scraping/page_trackers/{tracker_id}`             | Delete a tracker and all its revisions.                                     |
| `POST`   | `/api/web_scraping/page_trackers/{tracker_id}/_history`    | Return the stored revision history.                                         |
| `POST`   | `/api/web_scraping/page_trackers/{tracker_id}/_clear`      | Clear all stored revisions.                                                 |
| `GET`    | `/api/web_scraping/page_trackers/{tracker_id}/_logs`       | Return per-execution logs and phase timings.                                |
| `POST`   | `/api/web_scraping/page_trackers/{tracker_id}/_clear_logs` | Clear the execution logs.                                                   |
| `GET`    | `/api/web_scraping/page_trackers/_logs_summary`            | Per-tracker health summary (last N runs, error rate).                       |
| `POST`   | `/api/web_scraping/page_trackers/_debug`                   | Run the extractor once without persisting; returns the full pipeline trace. |

Authenticate with the Kratos session cookie or
`Authorization: Bearer su_ak_<token>` (see `secutils-api-keys`).

## Create-tracker payload

`POST /api/web_scraping/page_trackers` accepts `PageTrackerCreateParams`:

```json
{
  "name": "Hacker News - Top Post",
  "enabled": true,
  "config": {
    "revisions": 5,
    "timeout": 30000,
    "job": { "schedule": "0 */15 * * * *" }
  },
  "target": {
    "extractor": "export async function execute(page) { await page.goto('https://news.ycombinator.com/'); const a = page.locator('.athing .titleline a').first(); return `[${await a.textContent()}](${await a.getAttribute('href')})`; }",
    "userAgent": null,
    "ignoreHttpsErrors": false
  },
  "notifications": true,
  "secrets": "none",
  "tagIds": []
}
```

Field notes:

- `config.revisions` is the rolling history depth (1..100). Older
  revisions are dropped on overflow.
- `config.timeout` is per-run wall clock in ms (default 30000, max
  120000).
- `config.job.schedule` is a 6-field cron expression (seconds, minutes,
  hours, day-of-month, month, day-of-week). Omit `job` to make the
  tracker on-demand only.
- `target.extractor` must export `async function execute(page, context)`
  and return a serialisable value. The returned value is JSON-stringified
  (or kept as bytes if the script returns `Uint8Array`) and capped at
  1 MB.
- `target.userAgent` overrides the default Chromium UA when set.
- `secrets` follows the same `none` / `all` / `{ "selected": [...] }`
  union used elsewhere (see `secutils-secrets`). Secrets are exposed via
  the second argument: `execute(page, context)` with
  `context.params.secrets.MY_KEY`.

## Debug-without-saving

`POST /api/web_scraping/page_trackers/_debug` accepts the same
`target` and `config` fields and returns the full pipeline trace:
navigation timings, console messages, the extractor's return value, and
any thrown error. Use it from agents to validate an extractor against a
URL before persisting the tracker.

## Resource extraction shortcut

The Retrack scraper ships a helper that returns
`Array<{ type: 'script' | 'stylesheet', url?: string, content: { size: number, digest: string } }>`
for the visited page. The human guide includes a full example; in short
the extractor returns the result of `Deno.core.ops.op_extract_resources()`
to get a stable list suitable for change detection.

## Example flow (curl)

```bash
TRACKER=$(curl -sX POST https://secutils.dev/api/web_scraping/page_trackers \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d @page-tracker.json)
ID=$(echo "$TRACKER" | jq -r '.id')

# Read the latest revision (sorted newest first)
curl -sX POST "https://secutils.dev/api/web_scraping/page_trackers/$ID/_history" \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d '{"refresh":false}' | jq '.[0]'
```

## Caveats

- Pages protected by CAPTCHA, web-application firewalls, or strict bot
  detection (`navigator.webdriver` checks) often fail. Use an API
  tracker against an internal endpoint when possible.
- The extractor cannot make outbound HTTP requests (`fetch`,
  `XMLHttpRequest`); use `page.request` for HTTP, or proxy through a
  Secutils responder.
- `config.job.schedule` is on a best-effort scheduler shared across
  users. Free tier minimum interval is 1 hour; paid tiers allow shorter
  intervals.
- Notifications are sent through the email channel configured on the
  account; if the user has not verified an email address, notification
  delivery silently fails.
- The recording-import shortcuts (`Right-click → Import: Playwright
  recording / Chrome DevTools recording`) are UI-only; the API expects a
  fully formed `execute()` function in `target.extractor`.

## See also

- Human-readable guide: <https://secutils.dev/docs/guides/web_scraping/page>
- Related skill: `secutils-web-scraping-api` (HTTP-only tracker; lighter
  and faster when a browser is unnecessary)
- Sandbox API reference: `secutils-deno-runtime`
- OpenAPI: <https://secutils.dev/api-docs/openapi.json>
