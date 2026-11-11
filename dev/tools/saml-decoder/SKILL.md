---
name: saml-decoder
description: >-
  Decode and inspect Base64-encoded SAML responses, AuthnRequests, or
  metadata in the browser with the Secutils.dev SAML Decoder. Build a
  one-click prefilled URL the user can open by encoding the raw SAML payload
  string into the fragment of https://tools.secutils.dev/saml#{encoded}.
  Trigger when the user asks to "decode this SAML response", "inspect a SAML
  AuthnRequest", "view SAML metadata", or anything that names
  secutils.dev/saml.
---

# SAML Decoder (Secutils.dev)

Hand the user a URL that opens the SAML Decoder pre-loaded with their payload.
Decoding, attribute extraction, and pretty-printing all happen in the browser
- the fragment is never sent to the server, so identifiers and claims stay
client-side. Both HTTP-POST (Base64) and HTTP-Redirect (Base64 + raw deflate)
binding payloads are auto-detected.

## Inputs

| Field  | Type   | Default  | Notes                                                                                                                                                           |
|--------|--------|----------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `saml` | string | required | Base64-encoded SAML (HTTP-POST body, HTTP-Redirect query, or raw XML). URL-encoded values are accepted; deflate-compressed Redirect payloads are auto-inflated. |

State shape: a **raw string** - no JSON wrapper. The state value is exactly
the SAML string the user supplied (after stripping any surrounding quotes /
whitespace).

## Wire format

After URL-safe base64 decoding:

```
| 4 bytes uncompressed-length (LE u32) | N bytes raw DEFLATE of UTF-8 string |
```

Pipeline: take the SAML string verbatim → UTF-8 bytes → `deflate-raw` →
prepend the 4-byte LE u32 of the **uncompressed** length → base64url
(`+`→`-`, `/`→`_`, strip `=`).

## How to produce the URL

Run this on any machine with Node ≥ 18 (no deps):

```bash
node -e '
const zlib = require("node:zlib");
const saml = process.argv[1];
const utf8 = Buffer.from(saml, "utf8");
const out = Buffer.concat([Buffer.alloc(4), zlib.deflateRawSync(utf8)]);
out.writeUInt32LE(utf8.length, 0);
const enc = out.toString("base64").replace(/\+/g,"-").replace(/\//g,"_").replace(/=+$/,"");
console.log("https://tools.secutils.dev/saml#" + enc);
' 'PHNhbWxwOlJlc3BvbnNlIHhtbG5zOnNhbWxwPSJ1cm46b2FzaXM6bmFtZXM6dGM6U0FNTDoyLjA6cHJvdG9jb2wiPi4uLjwvc2FtbHA6UmVzcG9uc2U+'
```

Pass the SAML payload as the first argv (single-quoted so embedded `+` and
`=` survive untouched). The fragment is opaque - print the full URL.

The same `tools.secutils.dev/saml` URL with **no fragment** loads an empty
decoder if you'd rather just direct the user to paste themselves.

## Decoding without opening the tool (optional)

If the user only wants the parsed assertion in chat (no UI), you can decode
the SAML yourself: `atob(saml)` for POST-binding payloads,
`pako.inflateRaw(atob(saml))` for Redirect-binding ones, then read the XML.
The wire format above is independent of that - it's only how state gets into
the configurator UI.

## After producing

Hand the URL back in a fenced block or inline code. Keep summaries to one
sentence; don't paraphrase the assertion contents.

## Caveats

- The SAML payload travels **in the URL fragment**. The fragment never
  reaches the Secutils server but is fully visible to anyone with the link.
  Real production SAML often carries email addresses, group memberships, and
  other PII - never share these URLs in public channels.
- Strip URL-encoding (`%2B`, `%3D`, …) before encoding; the tool accepts
  pre-decoded payloads more reliably.
- Signature verification requires the IdP's signing certificate and is
  **out of scope for this tool**. Use a server-side library for that.
