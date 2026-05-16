---
name: secutils-private-keys
description: >-
  Generate, list, update, delete, and export cryptographic private keys on
  Secutils.dev. Supports RSA (1024 to 8192 bits), DSA, ECDSA (P-256, P-384,
  P-521), and Ed25519, with optional passphrase encryption and export to
  PKCS#8 (DER or PEM) or legacy PEM. Trigger when the user asks to "generate
  an RSA key", "create an Ed25519 private key", "export a private key as
  PKCS#8", produce a key for use with a Secutils certificate template, or
  manage password-protected private keys for testing TLS, JWT, or other
  asymmetric cryptography. Requires an authenticated Kratos session cookie
  or a Secutils API key.
---

# Secutils.dev: Private Keys

The Private Keys utility stores user-generated asymmetric keypairs on the
server, optionally encrypted with a user-chosen passphrase. Keys can be
exported on demand in PKCS#8 DER, PKCS#8 PEM, or legacy PEM, and can be
used as the keypair behind a Secutils certificate template (see
`secutils-certificate-templates`). The guide lives at
<https://secutils.dev/docs/guides/digital_certificates/private_keys> and
the full API reference at <https://secutils.dev/api-docs/openapi.json>.

## Endpoints (tag: `certificates`)

| Method   | Path                                              | Purpose                                             |
|----------|---------------------------------------------------|-----------------------------------------------------|
| `GET`    | `/api/certificates/private_keys`                  | List all private keys for the current user.         |
| `GET`    | `/api/certificates/private_keys/{key_id}`         | Get a single private key's metadata.                |
| `POST`   | `/api/certificates/private_keys`                  | Generate a new keypair (`PrivateKeysCreateParams`). |
| `PUT`    | `/api/certificates/private_keys/{key_id}`         | Rename a key or change/remove its passphrase.       |
| `DELETE` | `/api/certificates/private_keys/{key_id}`         | Permanently delete a key.                           |
| `POST`   | `/api/certificates/private_keys/{key_id}/_export` | Export the key bytes in the requested format.       |

The list endpoint never returns key material; only metadata
(`id`, `name`, `alg`, `encrypted`, `createdAt`, `updatedAt`, `tags`) is
exposed. The raw key bytes are only available via `_export`.

## Authentication

Use either the `ory_kratos_session` cookie obtained from
<https://secutils.dev/signin> or an `Authorization: Bearer su_ak_<token>`
header from an API key created in `Settings → Security`
(see `secutils-api-keys`).

## Create-key payload

`POST /api/certificates/private_keys` accepts `PrivateKeysCreateParams`
(camelCase JSON):

```json
{
  "keyName": "my-key",
  "alg": { "keyType": "ed25519" },
  "passphrase": null,
  "tagIds": []
}
```

`alg` is a tagged union driven by `keyType`:

| `keyType`   | Required extras | Valid values                                |
|-------------|-----------------|---------------------------------------------|
| `"rsa"`     | `keySize`       | `"1024"`, `"2048"`, `"4096"`, `"8192"`      |
| `"dsa"`     | `keySize`       | `"1024"`, `"2048"`, `"4096"`                |
| `"ecdsa"`   | `curve`         | `"secp256r1"`, `"secp384r1"`, `"secp521r1"` |
| `"ed25519"` | (none)          | --                                          |

Notes:

- `keySize` is sent as a string, not an integer (matches the OpenAPI
  schema enum).
- `passphrase` is optional. When present the server encrypts the key at
  rest with AES-256-GCM; export must then supply the matching passphrase.
  When omitted the key is stored unencrypted on the server but never
  surfaced through any list or get endpoint.
- `tagIds` is the list of Workspace tag IDs to associate (see
  `secutils-tags`).

`POST` returns the newly created `PrivateKey` with its server-assigned
`id`.

## Export payload

`POST /api/certificates/private_keys/{key_id}/_export` body:

```json
{
  "format": "pkcs8",            // "pkcs8" | "pem"
  "passphrase": "current-pass", // required iff the key was created with one
  "exportPassphrase": "new-pass" // optional; if set, re-encrypts on export
}
```

Successful response is `application/octet-stream` containing the raw key
bytes. `pkcs8` produces a binary PKCS#8 DER blob (typical filename
`Key.p8`), `pem` produces a base64-armoured PEM (`-----BEGIN ...-----`).
If `exportPassphrase` is supplied the exported file is encrypted with that
passphrase regardless of how the key was stored, which is the standard way
to share an encrypted key with another user.

## Example flow (curl)

```bash
# Generate an Ed25519 key without a passphrase
curl -sX POST https://secutils.dev/api/certificates/private_keys \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d '{"keyName":"ed25519-test","alg":{"keyType":"ed25519"},"tagIds":[]}'

# Generate a 2048-bit RSA key, passphrase-protected
KEY=$(curl -sX POST https://secutils.dev/api/certificates/private_keys \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d '{"keyName":"rsa-2048","alg":{"keyType":"rsa","keySize":"2048"},"passphrase":"s3cret","tagIds":[]}')
ID=$(echo "$KEY" | jq -r '.id')

# Export as PEM, decrypted, for use with openssl
curl -sX POST "https://secutils.dev/api/certificates/private_keys/$ID/_export" \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d '{"format":"pem","passphrase":"s3cret"}' \
  -o rsa.pem
openssl pkey -in rsa.pem -text -noout
```

## Update vs regenerate

`PUT /api/certificates/private_keys/{key_id}` accepts a
`PrivateKeysUpdateParams` body that can change `keyName`, change the
`passphrase` (must include the existing `passphrase` to authenticate the
change), or update `tagIds`. There is no key rotation operation; if a key
needs new material, delete it and create a new one.

## Caveats

- Asymmetric algorithm choice cannot be changed after creation. Pick the
  right `alg` up front.
- The server never returns plaintext key bytes from `GET` endpoints. The
  only way to retrieve the bytes is `_export` with the correct passphrase.
- Deleting a key that is referenced by a certificate template only
  detaches the reference; the template then has no keypair until a new
  one is supplied.
- The `alg.keySize` value is a quoted string in the JSON schema (e.g.
  `"2048"`). Passing it as a JSON number returns `400 Bad Request`.
- For storing the matching certificate, generate a certificate template
  bound to this key with the `secutils-certificate-templates` skill.

## See also

- Human-readable guide: <https://secutils.dev/docs/guides/digital_certificates/private_keys>
- Related skill: `secutils-certificate-templates` (for X.509 issuance)
- OpenAPI: <https://secutils.dev/api-docs/openapi.json>
