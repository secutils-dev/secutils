---
name: secutils-csp
description: >-
  Create, edit, share, serialize, and import Content Security Policy
  templates on Secutils.dev. Policies can be authored as a structured set
  of directives, imported by URL (fetching the live `Content-Security-Policy`
  or `Content-Security-Policy-Report-Only` header or `<meta>` tag from any
  page), or imported from a serialized policy string. Each policy can be
  serialized back to a header value or an HTML `<meta http-equiv>` tag and
  publicly shared through a tokenised URL. Trigger when the user asks to
  build a CSP, generate a `Content-Security-Policy` header or meta tag,
  copy a site's CSP into Secutils for editing, share a CSP template, or
  test a CSP against a webhook responder.
---

# Secutils.dev: Content Security Policies

The CSP utility stores reusable Content Security Policy templates. Each
template owns a set of directives keyed by directive name (e.g.
`default-src`, `script-src`, `style-src`, `frame-ancestors`,
`upgrade-insecure-requests`) and a list of source expressions. A template
can be serialized into either an HTTP header value or an inline
`<meta http-equiv="Content-Security-Policy">` tag, and can be shared
read-only through a tokenised URL.

Imports can either fetch a live policy from a remote URL (header or
report-only header or meta tag) or parse a serialized policy string the
user supplies directly. The serialized representation round-trips
losslessly with the structured representation.

Guide: <https://secutils.dev/docs/guides/web_security/csp>.
Full reference: <https://secutils.dev/api-docs/openapi.json>.

## Endpoints (tag: `web_security`)

| Method   | Path                                           | Purpose                                                      |
|----------|------------------------------------------------|--------------------------------------------------------------|
| `GET`    | `/api/web_security/csp`                        | List all policies for the current user.                      |
| `GET`    | `/api/web_security/csp/{policy_id}`            | Read a single policy. Accepts an optional shared-link token. |
| `POST`   | `/api/web_security/csp`                        | Create a policy (`ContentSecurityPoliciesCreateParams`).     |
| `PUT`    | `/api/web_security/csp/{policy_id}`            | Replace the policy contents.                                 |
| `DELETE` | `/api/web_security/csp/{policy_id}`            | Delete a policy.                                             |
| `POST`   | `/api/web_security/csp/{policy_id}/_serialize` | Serialize the policy into a header or meta tag.              |
| `POST`   | `/api/web_security/csp/{policy_id}/_share`     | Create or rotate a public share token.                       |
| `POST`   | `/api/web_security/csp/{policy_id}/_unshare`   | Revoke the current share token.                              |

Authenticate with the Kratos session cookie or
`Authorization: Bearer su_ak_<token>` (see `secutils-api-keys`).

## Create-policy payload

`POST /api/web_security/csp` accepts `ContentSecurityPoliciesCreateParams`:

```json
{
  "name": "my-csp",
  "content": {
    "type": "serialized",
    "value": "default-src 'self'; script-src 'self' https://cdn.example.com"
  },
  "tagIds": []
}
```

`content` is a tagged union driven by `type`:

| `type`         | Extras                                                                     | Behaviour                                                    |
|----------------|----------------------------------------------------------------------------|--------------------------------------------------------------|
| `"serialized"` | `value: string`                                                            | Parse `value` as a complete serialized CSP policy.           |
| `"directives"` | `directives: Record<string, string[]>`                                     | Use the structured directives directly.                      |
| `"remote"`     | `url: string`, `source: "enforcingHeader" \| "reportOnlyHeader" \| "meta"` | Fetch the policy from the given URL using the chosen source. |

Successful `POST` returns the created `ContentSecurityPolicy` including its
normalised `directives` and computed enforcement metadata.

## Serialize payload

`POST /api/web_security/csp/{policy_id}/_serialize` body:

```json
{
  "source": "enforcingHeader"   // "enforcingHeader" | "reportOnlyHeader" | "meta"
}
```

Response:

```json
{ "serialized": "default-src 'self'; script-src 'self' ..." }
```

For `meta` the serialised value is an HTML snippet, e.g.
`<meta http-equiv="Content-Security-Policy" content="default-src 'self'">`.

## Example flow (curl)

```bash
# Import the CSP that google.com sends in report-only mode
CSP=$(curl -sX POST https://secutils.dev/api/web_security/csp \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d '{
    "name": "Google (report-only)",
    "content": { "type": "remote", "url": "https://www.google.com", "source": "reportOnlyHeader" },
    "tagIds": []
  }')
ID=$(echo "$CSP" | jq -r '.id')

# Render it as an HTML <meta> tag
curl -sX POST "https://secutils.dev/api/web_security/csp/$ID/_serialize" \
  -H "Authorization: Bearer $SECUTILS_API_KEY" \
  -H "content-type: application/json" \
  -d '{"source":"meta"}' | jq -r '.serialized'
```

## Caveats

- `remote` import follows redirects but does not execute JavaScript, so
  CSPs injected only via inline scripts will not be discovered. Use the
  `meta` source on the rendered HTML when the live page sets the policy
  through `<meta http-equiv>`.
- Directive order is preserved on round-trip but source expression order
  inside a directive is normalised to the canonical CSP grammar
  (keywords first, then schemes, then hosts).
- Sharing a CSP exposes only its directives, not its name or tags.
- To exercise a CSP end-to-end, pair this skill with `secutils-webhooks`:
  create a responder that returns an HTML page with the policy's meta
  tag in `<head>` and request it from a browser.

## See also

- Human-readable guide: <https://secutils.dev/docs/guides/web_security/csp>
- Related skill: `secutils-webhooks` (for hosting an HTML page that
  serves the policy as a header or meta tag)
- OpenAPI: <https://secutils.dev/api-docs/openapi.json>
