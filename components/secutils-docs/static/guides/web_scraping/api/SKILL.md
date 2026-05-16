---
name: secutils-web-scraping-api
description: >-
  Create, list, update, debug, and run API trackers on Secutils.dev. An
  API tracker is a scheduled HTTP request (GET, POST, PUT, PATCH, DELETE)
  against an HTTP or HTTPS endpoint, with optional JavaScript configurator
  and extractor scripts that run in a Deno sandbox to mutate the request
  pre flight and transform the response post flight. Stores up to N
  revisions of the extracted body and notifies on change. Trigger when the
  user asks to monitor a REST or JSON API for changes, alert on a status
  change in a public webhook, watch a third party API's response format,
  build a synthetic check, or test a JSON endpoint and compare runs.
  Lighter and faster than page trackers because no browser is launched.
---

# Secutils.dev: Web Scraping (API Trackers)

An API tracker issues an HTTP request to a URL on a cron schedule, runs
an optional `configurator` script to mutate the request before it goes
out (add headers, sign payloads, expand templated bodies), and an
optional `extractor` script to transform the response into the value
that gets persisted as the latest revision. Both scripts run inside the
Retrack Deno sandbox documented in `secutils-deno-runtime`.

Use API trackers whenever a browser is unnecessary: JSON APIs, status
endpoints, GraphQL queries, RSS feeds, plain-text endpoints. For pages
that need full DOM rendering or interaction, use the page tracker
documented in `secutils-web-scraping-page`.

Guide: <https://secutils.dev/docs/guides/web_scraping/api>.
Full reference: <https://secutils.dev/api-docs/openapi.json>.

## Endpoints (tag: `web_scraping`)

| Method   | Path                                                      | Purpose                                                                                           |
|----------|-----------------------------------------------------------|---------------------------------------------------------------------------------------------------|
| `GET`    | `/api/web_scraping/api_trackers`                          | List all API trackers.                                                                            |
| `POST`   | `/api/web_scraping/api_trackers`                          | Create a tracker (`ApiTrackerCreateParams`).                                                      |
| `PUT`    | `/api/web_scraping/api_trackers/{tracker_id}`             | Replace a tracker's configuration.                                                                |
| `DELETE` | `/api/web_scraping/api_trackers/{tracker_id}`             | Delete a tracker and all its revisions.                                                           |
| `POST`   | `/api/web_scraping/api_trackers/{tracker_id}/_history`    | Return the stored revision history.                                                               |
| `POST`   | `/api/web_scraping/api_trackers/{tracker_id}/_clear`      | Clear all stored revisions.                                                                       |
| `GET`    | `/api/web_scraping/api_trackers/{tracker_id}/_logs`       | Per-execution logs and phase timings.                                                             |
| `POST`   | `/api/web_scraping/api_trackers/{tracker_id}/_clear_logs` | Clear the execution logs.                                                                         |
| `GET`    | `/api/web_scraping/api_trackers/_logs_summary`            | Health summary across all API trackers.                                                           |
| `POST`   | `/api/web_scraping/api_trackers/_test`                    | One-shot run that does not persist; returns the raw response.                                     |
| `POST`   | `/api/web_scraping/api_trackers/_debug`                   | Same as `_test` but also runs the configurator and extractor and returns the full pipeline trace. |

Authenticate with the Kratos session cookie or
`Authorization: Bearer su_ak_<token>` (see `secutils-api-keys`).

## Create-tracker payload

`POST /api/web_scraping/api_trackers` accepts `ApiTrackerCreateParams`:

```json
{
  "name": "App state",
  "enabled": true,
  "config": {
    "revisions": 10,
    "timeout": 10000,
    "job": { "schedule": "0 0 * * * *" }
  },
  "target": {
    "requests": [
      {
        "url": "https://api.example.com/state",
        "method": "POST",
        "headers": { "content-type": "application/json" },
        "body": "{\"key\":\"value\"}",
        "acceptInvalidCertificates": false
      }
    ],
    "configurator": null,
    "extractor": null
  },
  "notifications": true,
  "secrets": "none",
  "tagIds": []
}
```

Field notes:

- `target.requests` is an array; the request at index 0 is the primary
  request. When the array has more than one entry, all requests are
  issued in order and the extractor sees every response in
  `context.responses`.
- `target.requests[i].body` is sent verbatim as the HTTP body. JSON
  bodies must be pre-stringified.
- `target.requests[i].acceptInvalidCertificates` disables TLS
  verification for self-signed development endpoints; do not enable in
  production.
- `target.configurator` is an IIFE `(() => { ... })()` that returns a
  mutated `requests` array. Use it to inject `Authorization` headers
  from `context.params.secrets.*`, sign payloads, or expand templated
  URLs from secrets.
- `target.extractor` is an IIFE that receives
  `context.responses: Array<{ status, headers, body: number[] }>` and
  returns `{ body: Uint8Array }` (or any of the auto-converted forms; see
  `secutils-deno-runtime`). When omitted, the raw body of the last
  response is stored.
- `secrets` follows the same `none` / `all` / `{ "selected": [...] }`
  union used by responders and page trackers (see `secutils-secrets`).

## Configurator and extractor contracts

### Configurator script

```javascript
(() => {
  const token = context.params?.secrets?.AUTH_TOKEN ?? '';
  return {
    requests: context.requests.map(r => ({
      ...r,
      headers: { ...r.headers, authorization: `Bearer ${token}` }
    }))
  };
})();
```

### Extractor script

```javascript
(() => {
  const last = context.responses?.[context.responses.length - 1];
  if (!last) return { body: Deno.core.encode('no response') };
  const json = JSON.parse(Deno.core.decode(new Uint8Array(last.body)));
  return { body: Deno.core.encode(JSON.stringify({ status: json.status })) };
})();
```

## Debug / test without saving

`POST /api/web_scraping/api_trackers/_debug` accepts the same
`target`, `config`, and `secrets` fields as the create endpoint and
returns:

```json
{
  "configurator": { "result": { ... }, "params": { ... }, "error": null },
  "requests":     [ { "request": { ... }, "response": { ... } } ],
  "extractor":    { "result": { ... }, "params": { ... }, "error": null },
  "result":       { "body": "..." },
  "elapsedMs":    123
}
```

`_test` is the lighter variant: it issues the HTTP request without
running the configurator or extractor and returns the raw response.
Agents should prefer `_debug` when validating an end-to-end pipeline.

## Example flow (curl)

```bash
# Create a JSON tracker
TRACKER=$(curl -sX POST https://secutils.dev/api/web_scraping/api_trackers \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d @api-tracker.json)
ID=$(echo "$TRACKER" | jq -r '.id')

# Newest revision first
curl -sX POST "https://secutils.dev/api/web_scraping/api_trackers/$ID/_history" \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d '{"refresh":true}' | jq '.[0]'
```

## Caveats

- The Deno sandbox does not expose `fetch`; requests are made by the
  Retrack runtime based on `target.requests`, not from inside the
  scripts. Configurators can only declare what to send; extractors can
  only read what came back.
- Compressed responses (gzip, deflate, brotli) are decompressed before
  the extractor sees them, so `JSON.parse(Deno.core.decode(...))` works
  regardless of `Content-Encoding`.
- Free tier minimum schedule interval is 1 hour; paid tiers allow
  shorter intervals.
- Storage is capped at 1 MB per revision; larger responses are
  truncated by the extractor (or by the default extractor when none is
  provided).
- A tracker with `secrets: "none"` running a configurator that tries
  to read `context.params.secrets.X` will receive `undefined`, not
  throw. Always coalesce with `?? ''` to avoid producing malformed
  request payloads.

## See also

- Human-readable guide: <https://secutils.dev/docs/guides/web_scraping/api>
- Related skill: `secutils-web-scraping-page` (full headless browser)
- Sandbox API reference: `secutils-deno-runtime`
- OpenAPI: <https://secutils.dev/api-docs/openapi.json>
