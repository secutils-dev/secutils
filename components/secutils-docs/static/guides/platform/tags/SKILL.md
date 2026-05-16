---
name: secutils-tags
description: >-
  Manage workspace tags on Secutils.dev. Tags are user-scoped, coloured
  labels (max 50 per user, max 20 per item) used to organise and filter
  responders, page trackers, API trackers, CSP policies, certificate
  templates, private keys, scripts, and secrets. Every create/update
  payload on those entities accepts a `tagIds` array of tag UUIDs that
  comes from this skill. Trigger when the user asks to "tag this
  responder", "filter by tag", create a new tag colour, rename or delete
  a tag, or build up a tagged workspace via the API.
---

# Secutils.dev: Tags

Tags are workspace-wide labels that any other entity (responder, page
tracker, API tracker, CSP, certificate template, private key, user
script, user secret) can reference through its `tagIds` array. Each tag
has a unique lowercased name (max 50 chars), a hex colour, and a
server-assigned UUID. The UI exposes per-page tag filters (OR logic) and
a global scope filter in the workspace header (AND logic); both are
purely client-side queries against the `tags` field on each entity, so
adding or removing tags through the API has the same effect as editing
through the UI.

Guide: <https://secutils.dev/docs/guides/platform/tags>.
Full reference: <https://secutils.dev/api-docs/openapi.json>.

## Endpoints (tag: `user`)

| Method   | Path                      | Purpose                                                        |
|----------|---------------------------|----------------------------------------------------------------|
| `GET`    | `/api/user/tags`          | List all tags for the current user.                            |
| `POST`   | `/api/user/tags`          | Create a tag (`UserTagsCreateParams`).                         |
| `PUT`    | `/api/user/tags/{tag_id}` | Rename or recolour an existing tag.                            |
| `DELETE` | `/api/user/tags/{tag_id}` | Delete a tag and remove it from every item that referenced it. |

Authenticate with the Kratos session cookie or
`Authorization: Bearer su_ak_<token>` (see `secutils-api-keys`).

## Create-tag payload

```json
{
  "name": "production",
  "color": "#54B399"
}
```

Field notes:

- `name` is normalised server-side: leading/trailing whitespace is
  trimmed and the result is lowercased. Duplicate names per user return
  `400 Bad Request`.
- `color` must be a 7-character hex string starting with `#`. The UI
  exposes a palette but any valid hex value is accepted.

Successful response is the created `UserTag` with its server-assigned
`id`. Use that `id` in subsequent `tagIds` arrays on responders,
trackers, CSP policies, certificate templates, private keys, scripts,
and secrets.

## Update-tag payload

```json
{ "name": "staging", "color": "#F1D86F" }
```

Both fields are optional; omit a field to leave it unchanged. Renaming
preserves the tag's `id`, so existing references on tagged entities
continue to work without modification.

## Delete behaviour

`DELETE /api/user/tags/{tag_id}` returns `204 No Content` and
transactionally removes the tag from every entity that referenced it.
There is no soft delete, so script the operation only when the loss of
the label is intentional.

## Example flow (curl)

```bash
# Create a tag
TAG=$(curl -sX POST https://secutils.dev/api/user/tags \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d '{"name":"production","color":"#54B399"}')
TAG_ID=$(echo "$TAG" | jq -r '.id')

# Create a responder pre-tagged with it
curl -sX POST https://secutils.dev/api/webhooks/responders \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d "$(jq -n --arg tid "$TAG_ID" '{
    name: "prod-mock",
    location: {pathType: "=", path: "/"},
    method: "GET",
    enabled: true,
    settings: {requestsToTrack: 5, statusCode: 200, body: "ok", secrets: "none"},
    tagIds: [$tid]
  }')"
```

## Caveats

- Hard limits enforced server-side: 50 tags per user, 20 tags per item.
  Both are surfaced via the OpenAPI schema and return `400` on overflow.
- The UI's per-page tag filter uses OR logic; the workspace scope
  filter uses AND logic. Both are purely client-side; the API always
  returns the full set of items the user owns.
- Tags participate in export/import (see `secutils-export-import`).
  Importing a tag with a name that already exists reuses the existing
  tag's `id`; the colour from the import file is ignored.

## See also

- Human-readable guide: <https://secutils.dev/docs/guides/platform/tags>
- Related skills: `secutils-webhooks`, `secutils-private-keys`,
  `secutils-csp`, `secutils-web-scraping-page`,
  `secutils-web-scraping-api`, `secutils-secrets`,
  `secutils-user-scripts`, `secutils-certificate-templates`
- OpenAPI: <https://secutils.dev/api-docs/openapi.json>
