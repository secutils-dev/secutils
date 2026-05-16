---
name: secutils-webhooks
description: >-
  Create, list, update, and delete webhook responders on Secutils.dev. A
  responder is a programmable HTTP endpoint hosted on the caller's dedicated
  subdomain (`<handle>.webhooks.secutils.dev/<path>`) that returns either a
  static body or a dynamic response built by a JavaScript script running in
  a Deno sandbox. Trigger when the user asks to mock an HTTP endpoint,
  simulate a webhook callback, build a honeypot, proxy or intercept upstream
  HTTP traffic, capture incoming requests for inspection, generate dynamic
  responses based on the request query or body, or expose a temporary HTML
  or JSON endpoint without writing a server. Requires an authenticated
  Kratos session cookie or an API key created from the Secutils.dev
  Settings page.
---

# Secutils.dev: Webhooks (Responders)

A responder is a Secutils-hosted HTTP endpoint on the user's randomly
assigned subdomain (`https://<handle>.webhooks.secutils.dev/<path>`). Each
responder owns one path, one HTTP method (or `ANY`), a default status code,
headers, and body. Advanced settings can attach a JavaScript script that
runs inside a restricted Deno sandbox for every incoming request and can
override the status, headers, or body, proxy the request to a real upstream,
or render dynamic HTML.

Captured request history (up to `requestsToTrack`) is available through the
`_history` endpoint and the UI grid, so a responder also works as a
lightweight HTTP traffic inspector. The human-readable guide lives at
<https://secutils.dev/docs/guides/webhooks> and the full API reference at
<https://secutils.dev/api-docs/openapi.json>.

## Endpoints (tag: `webhooks`)

| Method   | Path                                               | Purpose                                                |
|----------|----------------------------------------------------|--------------------------------------------------------|
| `GET`    | `/api/webhooks/responders`                         | List all responders for the current user.              |
| `POST`   | `/api/webhooks/responders`                         | Create a new responder (`RespondersCreateParams`).     |
| `PUT`    | `/api/webhooks/responders/{responder_id}`          | Replace responder settings (`RespondersUpdateParams`). |
| `DELETE` | `/api/webhooks/responders/{responder_id}`          | Permanently delete the responder and its history.      |
| `GET`    | `/api/webhooks/responders/{responder_id}/_history` | Return the captured request history.                   |
| `POST`   | `/api/webhooks/responders/{responder_id}/_clear`   | Clear the captured request history.                    |
| `GET`    | `/api/webhooks/responders/_stats`                  | Aggregate stats across all responders.                 |

## Authentication

Two equivalent options:

1. **Session cookie** (browser flow): authenticate through Kratos at
   `https://secutils.dev/signin` and reuse the resulting `ory_kratos_session`
   cookie on every API call.
2. **API key** (recommended for scripts and agents): create one in
   `Settings → Security → Manage API keys` (see the `secutils-api-keys`
   skill), then send it as `Authorization: Bearer su_ak_<token>`. API keys
   work without cookies and can authenticate every non-API-key-management
   endpoint.

## Create-responder payload

`POST /api/webhooks/responders` accepts `RespondersCreateParams` with the
following shape (camelCase JSON):

```json
{
  "name": "my-responder",
  "location": { "pathType": "=", "path": "/my-hook", "subdomainPrefix": null },
  "method": "ANY",
  "enabled": true,
  "settings": {
    "requestsToTrack": 10,
    "statusCode": 200,
    "body": "Hello",
    "headers": [["content-type", "text/plain"]],
    "script": null,
    "secrets": "none"
  },
  "tagIds": []
}
```

Field notes:

- `location.pathType` is `"="` (exact match), `"^"` (prefix match), or
  `"*"` (any sub-path).
- `location.subdomainPrefix` is optional and lets a single user own
  multiple subdomains under their handle.
- `method` is one of `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`,
  `OPTIONS`, `CONNECT`, `TRACE`, or `ANY`.
- `settings.requestsToTrack` is the rolling history depth (`0` disables
  capture).
- `settings.headers` is an array of two-string tuples, not an object, to
  preserve duplicate header names and ordering.
- `settings.script` is the JS body (without an outer IIFE wrapper, the
  runtime adds one). See the `secutils-deno-runtime` skill for the
  sandbox API surface and the `context` shape available to the script.
- `settings.secrets` is `"none"`, `"all"`, or `{ "selected": [<secret_id>] }`
  and controls what `context.secrets.*` exposes (see the
  `secutils-secrets` skill).

The full response is the created `Responder` including its server-assigned
`id`, `createdAt`, `updatedAt`, and the absolute live URL the responder
serves.

## Example flow (curl)

```bash
# Create a JSON-mock responder
RESPONDER=$(curl -sX POST https://secutils.dev/api/webhooks/responders \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "name": "json-mock",
    "location": { "pathType": "=", "path": "/orders/123" },
    "method": "GET",
    "enabled": true,
    "settings": {
      "requestsToTrack": 5,
      "statusCode": 200,
      "headers": [["content-type", "application/json"]],
      "body": "{\"status\":\"shipped\"}",
      "secrets": "none"
    },
    "tagIds": []
  }')
echo "$RESPONDER" | jq -r '.url'   # the public webhook URL

# Read tracked requests
ID=$(echo "$RESPONDER" | jq -r '.id')
curl -s -H "Authorization: Bearer $SECUTILS_API_KEY" \
  "https://secutils.dev/api/webhooks/responders/$ID/_history" | jq
```

## Script-powered responders

When `settings.script` is set, the body and headers in `settings` become
the default response and the script can override them per request by
returning a `ScriptResult`:

```ts
interface ScriptResult {
  statusCode?: number;
  headers?: Record<string, string>;
  body?: Uint8Array | string | object;
  skipRequest?: boolean;
  trackResponse?: boolean;
}
```

The script receives a `context` global with the request method, path,
parsed headers, parsed query, raw body bytes, and decrypted secrets. The
full `context` interface, body auto-conversion table, and the
`Deno.core.ops.op_proxy_request()` API for safely forwarding requests
upstream are documented in the `secutils-deno-runtime` skill and in
`https://secutils.dev/docs/guides/webhooks#annex-responder-script-examples`.

## Caveats

- Responder URLs are public by design. There is no built-in auth wall.
  The dynamic-script section of the human guide includes self-contained
  HTTP Basic and cookie-session patterns that the script can apply when
  needed.
- The script body must not exceed roughly 50 KB. Reusable helpers should
  live in a user script (see the `secutils-user-scripts` skill) and be
  imported through the Workspace UI; the API stores the inlined,
  fully-expanded body.
- Setting `settings.body` together with a script that always returns a
  full `ScriptResult` is fine but wasteful: the static default is only
  used when the script returns `null`. Returning `null` keeps the default
  response intact, which is the pattern used by the auth-gate examples.
- Deleting a responder also deletes its captured request history. There
  is no soft delete.

## See also

- Human-readable guide: <https://secutils.dev/docs/guides/webhooks>
- Sandbox API reference: <https://secutils.dev/docs/guides/platform/deno_runtime>
- OpenAPI: <https://secutils.dev/api-docs/openapi.json>
