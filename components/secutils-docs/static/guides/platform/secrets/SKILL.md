---
name: secutils-secrets
description: >-
  Create, update, and delete encrypted user secrets on Secutils.dev.
  Secrets are write-only key-value pairs (max 10 KB per value, max 100
  per user) used to inject API tokens, passwords, private keys, or any
  other credential into responder scripts, page-tracker and API-tracker
  scripts, and responder static body/header templates without ever
  exposing the value to the browser after creation. Trigger when the
  user asks to store an API key for a Secutils responder, reference a
  credential from a tracker script, rotate a Secutils secret, or wire
  `${secrets.X}` template substitution into a static responder body.
---

# Secutils.dev: User Secrets

User secrets are write-only key-value pairs scoped to a single user.
The value is encrypted at rest with AES-256-GCM and is never returned
through any read endpoint. After creation the only way to read it is
indirectly: a responder script, page-tracker extractor, or API-tracker
configurator/extractor that was explicitly granted access can read the
decrypted value through its `context.secrets.*` (responders) or
`context.params.secrets.*` (trackers) global.

Static responder body and headers also support `${secrets.KEY}`
templating; the server expands the placeholder server-side at request
time and never exposes the literal value to the responder script even
when both mechanisms are in use.

Guide: <https://secutils.dev/docs/guides/platform/secrets>.
Full reference: <https://secutils.dev/api-docs/openapi.json>.

## Endpoints (tag: `user`)

| Method   | Path                            | Purpose                                                                                                |
|----------|---------------------------------|--------------------------------------------------------------------------------------------------------|
| `GET`    | `/api/user/secrets`             | List metadata for all secrets (id, name, createdAt, updatedAt).                                        |
| `POST`   | `/api/user/secrets`             | Create a new secret.                                                                                   |
| `PUT`    | `/api/user/secrets/{secret_id}` | Replace a secret's value (the only way to "rotate" it).                                                |
| `DELETE` | `/api/user/secrets/{secret_id}` | Delete a secret. Removes it from every responder/tracker that listed it in its `selected` access list. |

Authenticate with the Kratos session cookie or
`Authorization: Bearer su_ak_<token>` (see `secutils-api-keys`).

## Create-secret payload

```json
{
  "name": "THIRD_PARTY_API_KEY",
  "value": "sk-prod-..."
}
```

Naming rules (enforced server-side):

- Must start with a letter (`[a-zA-Z]`).
- May contain letters, digits, underscores, and hyphens.
- Max 128 characters.
- Must be unique per user.

`value` max length is 10 KB. Successful response is the created
`UserSecret` metadata (id, name, timestamps). The value field is never
echoed back.

## Update-secret payload

```json
{ "name": "THIRD_PARTY_API_KEY", "value": "sk-prod-new-..." }
```

Both fields are optional. Sending only `name` renames in place; sending
only `value` rotates the secret; sending both renames and rotates.

When a value rotates, Secutils automatically re-syncs the decrypted
value to every page tracker that lists this secret under its
`secrets: "all"` or `secrets: { "selected": [...] }` access list.
Responders resolve secrets at request time, so they always see the
latest value without any sync.

## Granting access to a responder or tracker

The `secrets` field on a responder, page tracker, or API tracker
controls which secrets are decrypted into its sandbox:

```json
"secrets": "none"                       // default - no secrets exposed
"secrets": "all"                        // every user secret is exposed
"secrets": { "selected": ["<id1>", "<id2>"] }   // explicit allow-list
```

For the `selected` mode the array contains the `id` field from
`GET /api/user/secrets` (not the `name`). Deleting a secret removes its
id from every `selected` list automatically.

## Accessing secrets at runtime

| Surface                               | Code                            |
|---------------------------------------|---------------------------------|
| Responder script                      | `context.secrets.MY_KEY`        |
| Responder static body or header value | `${secrets.MY_KEY}`             |
| Page tracker extractor                | `context.params.secrets.MY_KEY` |
| API tracker configurator or extractor | `context.params.secrets.MY_KEY` |

Missing or non-granted secrets resolve to `undefined` in scripts and
to the literal `${secrets.MY_KEY}` placeholder in static bodies (the
server leaves the unresolved placeholder in place so the failure is
visible to the consumer).

## Example flow (curl)

```bash
SECRET=$(curl -sX POST https://secutils.dev/api/user/secrets \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d '{"name":"THIRD_PARTY_API_KEY","value":"sk-prod-..."}')
SECRET_ID=$(echo "$SECRET" | jq -r '.id')

# Create a responder that's allowed to read this one secret
curl -sX POST https://secutils.dev/api/webhooks/responders \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d "$(jq -n --arg sid "$SECRET_ID" '{
    name: "third-party-mock",
    location: {pathType:"=", path:"/"},
    method: "GET",
    enabled: true,
    settings: {
      requestsToTrack: 5,
      statusCode: 200,
      headers: [["x-token","Bearer ${secrets.THIRD_PARTY_API_KEY}"]],
      body: "ok",
      secrets: {selected: [$sid]}
    },
    tagIds: []
  }')"
```

## Caveats

- Secrets are user-scoped, never workspace-scoped or shared. There is
  no "team secrets" tier.
- File-upload from the UI ends up as a UTF-8 string in `value`; for
  binary secrets, base64 the bytes before storing and decode inside
  the script.
- Rotating a secret that is referenced by a responder using static
  `${secrets.X}` template substitution takes effect on the next
  request; no re-deploy needed.
- The 10 KB value limit applies to the post-encryption byte count,
  which means base64-encoded binary blobs lose roughly 25% of capacity.

## See also

- Human-readable guide: <https://secutils.dev/docs/guides/platform/secrets>
- Related skills: `secutils-webhooks`, `secutils-web-scraping-page`,
  `secutils-web-scraping-api`, `secutils-deno-runtime`
- OpenAPI: <https://secutils.dev/api-docs/openapi.json>
