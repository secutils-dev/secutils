---
name: secutils-export-import
description: >-
  Export and import the entire workspace contents of a Secutils.dev
  account as a single `.secutils.json` file. Supports selective export of
  scripts, secrets, responders (with request history), certificate
  templates, private keys, content security policies, page trackers,
  API trackers, and user settings. Secret values can optionally be
  bundled, AES-256-GCM-encrypted with an Argon2id-derived key. Import
  runs in `merge` (additive, with per-collision rename/overwrite/skip)
  or `apply` (desired-state, can delete missing items) mode. Trigger
  when the user asks to back up their Secutils workspace, migrate
  between accounts, sync configuration-as-code from a Git repo, or
  detect drift against a known-good baseline.
---

# Secutils.dev: Export and Import

The export/import endpoints provide a single round-trip for the entire
workspace state. The on-the-wire payload is a versioned JSON document
(`{ version, exportedAt, data }`) with one top-level key per entity
collection. Export is read-only and selective; import is destructive
and stateful (it creates, updates, or deletes server-side entities to
match the supplied document).

Guide: <https://secutils.dev/docs/guides/platform/export_import>.
Full reference: <https://secutils.dev/api-docs/openapi.json>.

## Endpoints (tag: `user`)

| Method | Path                    | Purpose                                             |
|--------|-------------------------|-----------------------------------------------------|
| `POST` | `/api/user/data/export` | Generate and return a `.secutils.json` export.      |
| `POST` | `/api/user/data/import` | Apply a `.secutils.json` document to the workspace. |

Authenticate with the Kratos session cookie or
`Authorization: Bearer su_ak_<token>` (see `secutils-api-keys`).

## Export

`POST /api/user/data/export` accepts:

```json
{
  "include": {
    "scripts":               true,
    "secrets":               true,
    "responders":            true,
    "respondersHistory":     false,
    "certificateTemplates":  true,
    "privateKeys":           true,
    "contentSecurityPolicies": true,
    "pageTrackers":          true,
    "pageTrackersHistory":   false,
    "apiTrackers":           true,
    "apiTrackersHistory":    false,
    "settings":              true,
    "tags":                  true
  },
  "secretsPassphrase": null
}
```

Field notes:

- `include` is a flat boolean map; missing keys default to `false`.
- `respondersHistory`, `pageTrackersHistory`, and `apiTrackersHistory`
  add captured requests / past revisions to the export. They can
  dramatically increase file size; default to `false`.
- `secretsPassphrase` is required and validated (>= 8 chars) when
  `include.secrets` is true AND the caller also wants the encrypted
  values (the UI surfaces this as `Include secret values`). When
  omitted, only secret names are exported; values are dropped.

The response is the JSON document; standard `Content-Type:
application/json`. Save it as `*.secutils.json` for the human-readable
workflow.

Document shape:

```json
{
  "version": 1,
  "exportedAt": 1740000000,
  "data": {
    "scripts": [...],
    "secrets": [...],
    "tags": [...],
    "responders": [...],
    "certificateTemplates": [...],
    "privateKeys": [...],
    "contentSecurityPolicies": [...],
    "pageTrackers": [...],
    "apiTrackers": [...],
    "settings": { "common.uiTheme": "dark", ... }
  }
}
```

Only collections selected in `include` appear in `data`.

## Import

`POST /api/user/data/import` accepts:

```json
{
  "mode": "merge",                   // "merge" | "apply"
  "conflict": "rename",              // "rename" | "overwrite" | "skip"  (merge mode only)
  "secretsPassphrase": "user-chosen-passphrase",
  "document": { "version": 1, "exportedAt": 1740000000, "data": { ... } }
}
```

Field notes:

- `mode: "merge"` (default) is additive: items in the document are
  added on top of existing items. Conflicting names (same `name`
  inside the same collection) are resolved by `conflict`:
  - `"rename"` -> import with a ` (Copy N)` suffix.
  - `"overwrite"` -> replace the existing item; preserve its `id`.
  - `"skip"` -> keep the existing item; drop the imported one.
- `mode: "apply"` treats the document as the desired state. Items in
  the workspace that are absent from the document are deleted. The
  server returns a preview of what will be deleted; the caller must
  re-issue the request with `confirmDeletions: true` to actually
  destroy data. **Use with care; this mode can destroy years of
  history in a single request.**
- `secretsPassphrase` is required when the document contains
  encrypted secret values and must match the passphrase used at
  export. If the passphrase is missing or wrong, the import returns
  `400` with a clear error and no partial mutation is applied.
- Maximum import file size: 10 MB. Larger documents should be
  split or stripped of history.

Response is a summary of what was created, updated, skipped, renamed,
and deleted per collection, plus any per-entity errors.

## Example flow (curl)

```bash
# Take a full backup including secrets (encrypted)
curl -sX POST https://secutils.dev/api/user/data/export \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "include": {
      "scripts": true, "secrets": true, "responders": true,
      "certificateTemplates": true, "privateKeys": true,
      "contentSecurityPolicies": true, "pageTrackers": true,
      "apiTrackers": true, "settings": true, "tags": true
    },
    "secretsPassphrase": "correct horse battery staple"
  }' > workspace.secutils.json

# Restore (additive, rename on conflict)
jq -n --slurpfile doc workspace.secutils.json '{
  mode: "merge",
  conflict: "rename",
  secretsPassphrase: "correct horse battery staple",
  document: $doc[0]
}' | curl -sX POST https://secutils.dev/api/user/data/import \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" -d @-
```

## Caveats

- Tracker scheduling state (next-run, last-run) is not part of the
  export. After an import, every tracker is rescheduled relative to
  the import time.
- Captured request history (`respondersHistory`) is opaque and large.
  Inspecting it requires Secutils to re-render it; the JSON shape is
  preserved but not officially documented for downstream consumers.
- Subscription tier limits are enforced on import: an import that
  would push a collection past its tier limit is rejected and no
  partial state is left behind.
- `apply` mode does not delete user secrets that are not in the
  document, even though it deletes other missing entities. This is
  deliberate to prevent accidental loss of secret material that was
  never exported.

## See also

- Human-readable guide: <https://secutils.dev/docs/guides/platform/export_import>
- Related skill: `secutils-tags` (tags participate in export/import
  and are deduplicated by name on import)
- OpenAPI: <https://secutils.dev/api-docs/openapi.json>
