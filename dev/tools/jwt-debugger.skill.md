---
name: jwt-debugger
description: >-
  Decode, verify, and sign HMAC JSON Web Tokens (HS256, HS384, HS512) in the
  browser with the Secutils.dev JWT Debugger. Build a one-click prefilled URL
  the user can open by encoding `{j: <jwt>, s: <secret-or-empty>}` into the
  fragment of https://tools.secutils.dev/jwt#{encoded}. Trigger when the user
  asks to "decode this JWT", "verify a JWT signature", "sign a JWT with HS256",
  inspect a Bearer token, or anything that names secutils.dev/jwt.
---

# JWT Debugger (Secutils.dev)

Hand the user a URL that opens the JWT Debugger pre-loaded with their token
and (optional) secret. Decoding, verification, and signing all happen in the
browser - the fragment is never sent to the server. Asymmetric algorithms (RS,
ES, PS) are deliberately unsupported by this tool; for those, use a server-side
library and report results in chat.

## Inputs

| Field    | Type   | Default  | Notes                                                                                 |
|----------|--------|----------|---------------------------------------------------------------------------------------|
| `jwt`    | string | required | The compact-form token: `header.payload.signature`. Strip any `Bearer ` prefix first. |
| `secret` | string | `""`     | Optional HMAC secret used for verification or re-signing. Empty string when unknown.  |

State object shape (keys are exactly `j`, `s`):

```json
{"j":"<jwt-string>","s":"<secret-or-empty>"}
```

## Wire format

After URL-safe base64 decoding:

```
| 4 bytes uncompressed-length (LE u32) | N bytes raw DEFLATE of UTF-8 JSON |
```

Pipeline: `JSON.stringify(state)` → UTF-8 bytes → `deflate-raw` → prepend the
4-byte LE u32 of the **uncompressed** length → base64url (`+`→`-`, `/`→`_`,
strip `=`).

## How to produce the URL

Run this on any machine with Node ≥ 18 (no deps):

```bash
node -e '
const zlib = require("node:zlib");
const state = JSON.parse(process.argv[1]);
const utf8 = Buffer.from(JSON.stringify(state), "utf8");
const out = Buffer.concat([Buffer.alloc(4), zlib.deflateRawSync(utf8)]);
out.writeUInt32LE(utf8.length, 0);
const enc = out.toString("base64").replace(/\+/g,"-").replace(/\//g,"_").replace(/=+$/,"");
console.log("https://tools.secutils.dev/jwt#" + enc);
' '{"j":"eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c","s":"your-256-bit-secret"}'
```

Pass the state as the first argv (single-quoted JSON). The fragment is opaque
- print the full URL without splitting or abbreviating it.

The same `tools.secutils.dev/jwt` URL with **no fragment** loads an empty
debugger if you'd rather just direct the user to the tool to paste themselves.

## Decoding without opening the tool (optional)

If the user only wants the decoded payload in chat (no UI), do it yourself:
split on `.`, base64url-decode the first two segments to JSON, report them.
Signature verification needs the secret - run HMAC-SHA-256/384/512 over
`<header>.<payload>` and compare against the third segment.

## After producing

Hand the URL back in a fenced block or inline code. Keep summaries to one
sentence; don't paraphrase the payload contents.

## Caveats

- The JWT and secret travel **in the URL fragment**, which never reaches the
  Secutils server but is fully visible to anyone with the link. Never paste
  high-value production secrets into a share URL - generate or rotate the
  secret first.
- Strip `Bearer ` (and any surrounding whitespace) from the JWT before
  encoding; the tool expects the raw three-segment token.
- Asymmetric algorithms (RS*, ES*, PS*) are not supported by this tool. If
  the header `alg` is one of those, decode in chat and tell the user to use a
  server-side library for verification.
