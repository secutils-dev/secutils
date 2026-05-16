---
name: secutils-user-scripts
description: >-
  Create, list, update, duplicate, and delete reusable JavaScript user
  scripts on Secutils.dev. A user script is a snippet (max 50 KB)
  classified by `type` (`responder`, `api_extractor`, `api_configurator`,
  `page_extractor`, `universal`) that the workspace UI can import into
  any compatible editor with a single click. Imports inline the script
  content at import time, so later edits to the source script do not
  propagate. Trigger when the user asks to save a reusable Secutils
  snippet, build a shared library of responder logic, share a tracker
  extractor between multiple trackers, or manage the user-script
  catalogue from a script or AI agent.
---

# Secutils.dev: User Scripts

User scripts are reusable JavaScript snippets stored in the user's
workspace and surfaced through the "Import" action on every script
editor in the UI (responder scripts, API tracker configurator,
API/page tracker extractor). Each script has a `type` that constrains
which editors will list it for import; the `universal` type is offered
in every editor.

Importing a script copies its body verbatim into the destination
editor at import time. There is no live link back to the source
script, so a later edit to the source does not propagate to consumers.

Guide: <https://secutils.dev/docs/guides/platform/user_scripts>.
Full reference: <https://secutils.dev/api-docs/openapi.json>.

## Endpoints (tag: `user`)

| Method   | Path                            | Purpose                                    |
|----------|---------------------------------|--------------------------------------------|
| `GET`    | `/api/user/scripts`             | List all scripts.                          |
| `GET`    | `/api/user/scripts/{script_id}` | Read a single script's content.            |
| `POST`   | `/api/user/scripts`             | Create a script.                           |
| `PUT`    | `/api/user/scripts/{script_id}` | Replace a script's name, type, or content. |
| `DELETE` | `/api/user/scripts/{script_id}` | Delete a script.                           |

Authenticate with the Kratos session cookie or
`Authorization: Bearer su_ak_<token>` (see `secutils-api-keys`).

## Create-script payload

```json
{
  "name": "BasicAuthGate",
  "type": "responder",
  "content": "(() => { /* ... */ })();"
}
```

Field notes:

- `name` must start with a letter, may contain letters, digits,
  underscores, or hyphens, max 128 characters, unique per user.
- `type` is one of:
  - `responder` -> shown in responder script editors.
  - `api_extractor` -> shown in API tracker extractor editors.
  - `api_configurator` -> shown in API tracker configurator editors.
  - `page_extractor` -> shown in page tracker extractor editors.
  - `universal` -> shown in every editor.
- `content` is the raw script body, max 50 KB.

Successful response is the created `UserScript` including its
server-assigned `id`.

## Update vs duplicate

`PUT` accepts any subset of `name`, `type`, `content`. Changing
`type` is permitted but does not retroactively update any responder or
tracker that previously imported the script: imports are snapshots.

There is no dedicated duplicate endpoint; the UI's "Duplicate" action
is a client-side `GET` of the source followed by a `POST` with a new
`name` (the rest of the body is identical). Agents can mirror that
pattern.

## Example flow (curl)

```bash
# Create a universal helper
SCRIPT=$(curl -sX POST https://secutils.dev/api/user/scripts \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "name": "decode-json-body",
    "type": "universal",
    "content": "const body = context.body && context.body.length > 0 ? JSON.parse(Deno.core.decode(new Uint8Array(context.body))) : {};"
  }')
SCRIPT_ID=$(echo "$SCRIPT" | jq -r '.id')

# Read it back later
curl -s -H "Authorization: Bearer $SECUTILS_API_KEY" \
  "https://secutils.dev/api/user/scripts/$SCRIPT_ID" | jq -r '.content'
```

## Caveats

- Imports are snapshots; a fix in the source script does not flow to
  responders and trackers that already imported it. Use the search
  feature in the UI or a script that updates every consumer through
  the API to roll out a fix.
- The `responder` type is the only one that may run inside the
  responder request lifecycle; importing an `api_extractor` into a
  responder editor is not offered by the UI but the API does not
  enforce the constraint on the destination side. Choose `universal`
  for cross-editor helpers.
- Hard limits: 100 scripts per user (configurable per subscription
  tier), 50 KB per script.

## See also

- Human-readable guide: <https://secutils.dev/docs/guides/platform/user_scripts>
- Sandbox API reference: `secutils-deno-runtime`
- Consumers: `secutils-webhooks`, `secutils-web-scraping-page`,
  `secutils-web-scraping-api`
- OpenAPI: <https://secutils.dev/api-docs/openapi.json>
