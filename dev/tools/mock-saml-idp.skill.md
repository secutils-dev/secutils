---
name: mock-saml-idp-elastic
description: >-
  Generate signed SAML 2.0 responses to test Elasticsearch's SAML realm and
  Kibana's SAML auth provider end-to-end with the Secutils.dev Mock SAML IdP.
  Hand the user https://tools.secutils.dev/elastic/saml/idp-login, tell them
  to fill the form (username, roles, Kibana destination), and submit. Trigger
  when the user asks to "test Kibana SAML SSO", "generate a signed SAML
  response for Elasticsearch", "mock a SAML IdP for Elastic", or anything that
  names secutils.dev/elastic/saml/idp-login.
---

# Mock SAML IdP for Elasticsearch and Kibana (Secutils.dev)

Direct the user to the in-browser SAML IdP simulator. The tool generates a
signing key, builds a SAML 2.0 Response with the requested `NameID`,
attributes, and roles, signs it, and submits it to Kibana's
`/api/security/saml/callback`. Both flows are supported:

- **IdP-initiated**: open the page directly, fill the form, post to Kibana.
- **SP-initiated**: Kibana redirects to the page with `?SAMLRequest=...`;
  the form preserves the `RelayState` and posts back to the same Kibana host.

This tool is intentionally narrow (Elastic Stack only) and is **not** linked
from the marketing home page.

## Inputs (form fields, not URL state)

| Field         | Type          | Default                 | Notes                                                                       |
|---------------|---------------|-------------------------|-----------------------------------------------------------------------------|
| `username`    | string        | `test_user`             | The `NameID` of the SAML assertion.                                         |
| `fullname`    | string        | `Test User`             | Friendly display name attribute.                                            |
| `email`       | string        | `test_user@elastic.co`  | Email attribute.                                                            |
| `roles`       | array<string> | `[]`                    | Any of `viewer`, `editor`, `admin`, `superuser`, `kibana_admin`, or custom. |
| `destination` | string        | `http://localhost:5601` | Kibana host. Path is fixed to `/api/security/saml/callback`.                |

There is **no URL-state wire format** for this tool - inputs are entered in
the form on the page itself.

## How to direct the user

```
https://tools.secutils.dev/elastic/saml/idp-login
```

Then in the page:

1. Fill **Username**, **Full name**, **Email** (defaults are fine for a smoke
   test).
2. Pick role(s) from the dropdown, or type custom role strings.
3. Set **Destination** to the Kibana host they want to authenticate against.
4. Click **Sign in** to POST the signed assertion to Kibana.

For SP-initiated flows the user just clicks the SSO link in Kibana - the page
opens automatically with the right `RelayState` pre-filled.

## Inline alternative (no browser)

For purely scripted / CI testing, use a server-side library
(`python-saml`, `passport-saml`, the Elastic security testing utilities). This
tool is the right answer when the user wants to **drive a real Kibana
session** in the browser end-to-end with a controllable identity, including
seeing what `xpack.security.authc.realms.saml.*` settings actually accept.

## After producing

Once the user clicks **Sign in**, the browser POSTs to Kibana and Kibana
either lands them in the app (success) or shows a SAML error (mismatched
issuer, audience, or signing config). The tool itself doesn't need a follow-up
from the agent - the user iterates inside Kibana.

## Caveats

- The signing key is generated **client-side per page load** - close the tab
  and the certificate is gone. For repeatable tests, configure
  Elasticsearch's `xpack.security.authc.realms.saml.<realm>.idp.metadata.path`
  to a fixed file rather than letting it pick up the dynamic cert.
- The default `Issuer` is `urn:secutils-dev:saml-idp`. Match it in your
  Elasticsearch realm's `idp.entity_id` setting or the assertion will be
  rejected.
- Roles are mapped via Kibana's role-mapping rules - assigning `superuser`
  in this tool does nothing unless your `role_mapping.yml` (or the API
  equivalent) maps that group to the corresponding Elasticsearch role.
