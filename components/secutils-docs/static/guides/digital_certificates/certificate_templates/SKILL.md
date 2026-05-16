---
name: secutils-certificate-templates
description: >-
  Create, edit, share, and generate X.509 digital certificates from
  templates on Secutils.dev. A template captures the key algorithm,
  signature algorithm, validity window, distinguished name, key usage,
  extended key usage, basic constraints, and SANs, and produces self
  signed or CA signed bundles in PEM, PKCS#12 (PFX), or PKCS#8. Also
  exposes a one shot HTTPS endpoint fetcher that returns the live TLS
  chain of any remote URL as PEM. Trigger when the user asks to generate a
  test TLS certificate, build a self signed CA bundle, create a PFX for a
  Node.js or IIS HTTPS server, produce a JWK from a generated keypair,
  share a certificate template publicly, or fetch the cert chain that a
  real HTTPS server presents.
---

# Secutils.dev: Certificate Templates

A certificate template is a reusable specification for issuing an X.509
certificate (and its private key) on demand. The template stores everything
needed to (re)issue the bundle deterministically: key algorithm and
parameters, signature algorithm, version, validity dates, distinguished
name fields, basic constraints, key usage, extended key usage, and SANs.
Generation can produce PEM, PKCS#12 (PFX, optionally passphrase
protected), or a private-key-only PKCS#8 blob suitable for JWK export.
Templates can be shared publicly through a tokenised URL so other users
can clone them.

Guide: <https://secutils.dev/docs/guides/digital_certificates/certificate_templates>.
Full reference: <https://secutils.dev/api-docs/openapi.json>.

## Endpoints (tag: `certificates`)

| Method   | Path                                                  | Purpose                                                        |
|----------|-------------------------------------------------------|----------------------------------------------------------------|
| `GET`    | `/api/certificates/templates`                         | List all templates for the current user.                       |
| `GET`    | `/api/certificates/templates/{template_id}`           | Read a single template. Accepts an optional shared-link token. |
| `POST`   | `/api/certificates/templates`                         | Create a template (`TemplatesCreateParams`).                   |
| `PUT`    | `/api/certificates/templates/{template_id}`           | Replace the template's attributes.                             |
| `DELETE` | `/api/certificates/templates/{template_id}`           | Delete a template.                                             |
| `POST`   | `/api/certificates/templates/{template_id}/_generate` | Generate the certificate bundle.                               |
| `POST`   | `/api/certificates/templates/{template_id}/_share`    | Create or rotate a public share token.                         |
| `POST`   | `/api/certificates/templates/{template_id}/_unshare`  | Revoke the current share token.                                |
| `POST`   | `/api/certificates/_fetch`                            | Fetch the live TLS chain from a remote HTTPS URL as PEM.       |

Authenticate with the `ory_kratos_session` cookie or
`Authorization: Bearer su_ak_<token>` (see the `secutils-api-keys`
skill).

## Create-template payload

`POST /api/certificates/templates` accepts `TemplatesCreateParams`:

```json
{
  "templateName": "HTTPS Server",
  "attributes": {
    "keyAlgorithm": { "keyType": "rsa", "keySize": "2048" },
    "signatureAlgorithm": "sha256",
    "commonName": "localhost",
    "country": "US",
    "stateOrProvince": "California",
    "locality": "San Francisco",
    "organization": "CA Issuer, Inc",
    "organizationalUnit": null,
    "notValidBefore": 946720800,
    "notValidAfter": 1893456000,
    "version": 3,
    "isCa": false,
    "keyUsage": ["digitalSignature", "keyEncipherment"],
    "extendedKeyUsage": ["tlsWebServerAuthentication"],
    "subjectAlternativeNames": ["DNS:localhost", "IP:127.0.0.1"]
  },
  "tagIds": []
}
```

Field notes:

- `keyAlgorithm` follows the same tagged union used by the private-keys
  endpoint (`rsa`/`dsa`/`ecdsa`/`ed25519`; see `secutils-private-keys`).
- `signatureAlgorithm` is one of `sha1`, `sha256`, `sha384`, `sha512`,
  or `ed25519` (only `ed25519` is valid when `keyAlgorithm.keyType` is
  `ed25519`).
- `notValidBefore` / `notValidAfter` are Unix epoch seconds.
- `isCa: true` flips `basicConstraints.cA` and is required for the
  `keyCertSign` and `cRLSign` key usages.
- `keyUsage` is a subset of `digitalSignature`, `nonRepudiation`,
  `keyEncipherment`, `dataEncipherment`, `keyAgreement`, `keyCertSign`,
  `cRLSign`, `encipherOnly`, `decipherOnly`.
- `extendedKeyUsage` is a subset of `tlsWebServerAuthentication`,
  `tlsWebClientAuthentication`, `codeSigning`, `emailProtection`,
  `timeStamping`, `ocspSigning`.

## Generate-bundle payload

`POST /api/certificates/templates/{template_id}/_generate` body:

```json
{
  "format": "pkcs12",          // "pem" | "pkcs12" | "pkcs8"
  "passphrase": "pass"          // required for pkcs12 with encryption
}
```

Successful response is `application/octet-stream` with the bundle bytes.

- `pem` returns a concatenated `-----BEGIN PRIVATE KEY-----` and
  `-----BEGIN CERTIFICATE-----` block. Typical filename `bundle.pem`.
- `pkcs12` returns a PKCS#12 (`.pfx` / `.p12`) container. The
  `passphrase` field protects both the key bag and (when supported) the
  cert bag. Use it with Node.js `https.createServer({ pfx, passphrase })`.
- `pkcs8` returns the private key only, encoded as PKCS#8 DER. Useful as
  the input to a browser `crypto.subtle.importKey()` call when converting
  to JWK (the human guide walks through this exact recipe).

## Share / unshare

`POST /api/certificates/templates/{template_id}/_share` returns
`{ "url": "https://secutils.dev/share/<token>" }`. The URL is publicly
readable and renders the template in read-only mode; the token also
authenticates `GET /api/certificates/templates/{template_id}` requests
through the `?token=<token>` query parameter. `_unshare` revokes the
token and breaks any outstanding URLs.

## Fetch live chain

`POST /api/certificates/_fetch` with body
`{ "url": "https://example.com" }` returns `["-----BEGIN CERTIFICATE-----..."]`,
one PEM-encoded entry per chain certificate (leaf first, root last when
the server returns the full chain). Useful for inspecting a remote
deployment without `openssl s_client`. Invalid or unreachable URLs return
`400 Bad Request`.

## Example flow (curl)

```bash
TPL=$(curl -sX POST https://secutils.dev/api/certificates/templates \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d @template.json)
ID=$(echo "$TPL" | jq -r '.id')

# Generate a PFX bundle
curl -sX POST "https://secutils.dev/api/certificates/templates/$ID/_generate" \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d '{"format":"pkcs12","passphrase":"pass"}' \
  -o bundle.pfx

# Fetch a real server's chain
curl -sX POST https://secutils.dev/api/certificates/_fetch \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d '{"url":"https://www.google.com"}' | jq -r '.[]' > google-chain.pem
```

## Caveats

- The template owns its keypair. Calling `_generate` regenerates the key
  on every request unless the template was issued from a stored
  `secutils-private-keys` keypair. The UI deduplicates by hashing the
  inputs, but for fully deterministic issuance store the key separately
  and pin it.
- PFX export with `passphrase: ""` produces an unencrypted PKCS#12. Some
  consumers (notably curl on macOS) reject this. Prefer a non-empty
  passphrase.
- Sharing a template exposes its `attributes` but never any generated
  certificate or key material; recipients can clone the spec and
  regenerate locally.
- `notValidBefore` / `notValidAfter` are stored as epoch seconds; values
  outside the certificate's `Validity` ASN.1 type (1950..9999) are
  rejected.

## See also

- Human-readable guide: <https://secutils.dev/docs/guides/digital_certificates/certificate_templates>
- Related skill: `secutils-private-keys` (for pinning a long-lived keypair)
- OpenAPI: <https://secutils.dev/api-docs/openapi.json>
