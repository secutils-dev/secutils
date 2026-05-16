---
name: secutils-api-keys
description: >-
  Create, list, rename, regenerate, and delete API keys on Secutils.dev.
  API keys are opaque tokens prefixed `su_ak_` that authenticate
  programmatic access to every Secutils.dev REST endpoint without a
  browser session. Trigger when the user asks to "create a Secutils API
  key", needs to authenticate a script or AI agent against
  `https://secutils.dev/api/...`, wants to regenerate or rotate an
  existing key, or needs to manage key expiration. Note that API key
  management endpoints themselves cannot be invoked with an API key for
  bootstrap reasons; they require a Kratos session cookie.
---

# Secutils.dev: API Keys

API keys are opaque bearer tokens (`su_ak_` plus 64 hex characters)
that authenticate REST calls in place of an `ory_kratos_session`
cookie. They are the canonical authentication mechanism for scripts,
CI pipelines, and AI agents. Each key has a name, an optional
expiration date, a usage timestamp, and a single one-time-display
plaintext token returned only at creation or regeneration.

Guide: <https://secutils.dev/docs/guides/platform/api_keys>.
Full reference: <https://secutils.dev/api-docs/openapi.json>.

## Endpoints (tag: `user`)

| Method   | Path                                          | Purpose                                                           |
|----------|-----------------------------------------------|-------------------------------------------------------------------|
| `GET`    | `/api/user/api_keys`                          | List all API keys for the current user (metadata only).           |
| `POST`   | `/api/user/api_keys`                          | Create a new key; returns the plaintext token once.               |
| `PUT`    | `/api/user/api_keys/{api_key_id}`             | Rename a key.                                                     |
| `DELETE` | `/api/user/api_keys/{api_key_id}`             | Delete a key; the token stops working immediately.                |
| `POST`   | `/api/user/api_keys/{api_key_id}/_regenerate` | Generate a new token for the same name; old token is invalidated. |
| `POST`   | `/api/users/{user_id}/api_keys`               | (Operator only.) Bootstrap-create a key for another user.         |

**Important: every API-key-management endpoint above must be
authenticated with the `ory_kratos_session` cookie, not with an API
key.** The server returns `403 Forbidden` to an API-key-authenticated
request hitting `/api/user/api_keys/*`. This is deliberate: it
prevents a leaked API key from minting new tokens or rotating itself.
Authenticate from a browser session through Kratos first.

## Create-key payload

```json
{
  "name": "ci-pipeline",
  "expiresAt": 1893456000
}
```

`expiresAt` is optional Unix epoch seconds. Omit it to create a
non-expiring key.

Response:

```json
{
  "apiKey": {
    "id": "11111111-...",
    "name": "ci-pipeline",
    "expiresAt": 1893456000,
    "createdAt": 1700000000,
    "updatedAt": 1700000000,
    "lastUsedAt": null
  },
  "token": "su_ak_<64-hex>"
}
```

The `token` field is the only place the plaintext token is ever shown.
Store it in the agent's secret manager immediately. Subsequent `GET`
calls return only the metadata, never the token.

## Using a key

Send the token in the `Authorization` header on every non-management
endpoint:

```bash
curl -H "Authorization: Bearer su_ak_<token>" \
  https://secutils.dev/api/webhooks/responders
```

Any 401 indicates either an unknown token or an expired key; the server
distinguishes between the two through the `WWW-Authenticate` header.

## Regenerate vs delete

`POST .../_regenerate` accepts an optional `expiresAt` field and
returns the same shape as `POST /api/user/api_keys`, but the key's
`id` is preserved. Any consumer using the previous token loses access
immediately, so coordinate the rotation with the consumer's deploy.

`DELETE` removes the key outright; the `id` is freed and any consumer
using the token gets `401`.

## Caveats

- API keys grant the same scope as the owning user's session: every
  data endpoint, no API-key-management endpoints, no admin endpoints.
  Treat them as a full credential.
- Limits: 30 API keys per user (configurable via
  `security.max_user_api_keys`); name max 128 chars, unique per user.
- `lastUsedAt` updates on every authenticated request, with a 60-second
  coalescing window to avoid contention. A key that shows `null` for
  several minutes after first use has not been exercised.
- For ephemeral automation, prefer creating a short-lived API key with
  `expiresAt = now + 1h` over a never-expiring key.
- Bootstrap recipe: sign in through the UI, open the browser devtools,
  copy the `ory_kratos_session` cookie, then call
  `POST /api/user/api_keys` from a shell with that cookie. Subsequent
  agent calls use the returned token directly.

## See also

- Human-readable guide: <https://secutils.dev/docs/guides/platform/api_keys>
- Related skill: every other Secutils skill assumes an API key is
  already in `$SECUTILS_API_KEY`.
- OpenAPI: <https://secutils.dev/api-docs/openapi.json>
