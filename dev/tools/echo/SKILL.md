---
name: mock-response
description: >-
  Build a shareable mock HTTP response URL using the Secutils.dev echo tool -
  given a status code, headers, body, and optional description, encode the
  state and emit https://tools.secutils.dev/echo#{encoded} (configurator) or
  https://tools.secutils.dev/echo?c={encoded} (served mock). Trigger when the
  user asks for a "mock response", "echo URL", "fake HTTP response", "test
  response URL", or anything that names secutils.dev/echo.
---

# Mock response (Secutils.dev echo)

Produce a URL that opens (or serves, with `?c=`) a mock HTTP response on the
Secutils.dev echo responder. State is round-tripped through the URL itself; no
account or API call is involved.

## Inputs

| Field         | Type                     | Default | Notes                                                                                                                    |
|---------------|--------------------------|---------|--------------------------------------------------------------------------------------------------------------------------|
| `description` | string                   | `""`    | Free-text label, kept in URL, **not** sent to clients.                                                                   |
| `status`      | int 100–599              | `200`   | Clamp out-of-range values.                                                                                               |
| `headers`     | array of `[name, value]` | `[]`    | Drop entries with empty `name`. Responder auto-adds `Content-Type: text/plain; charset=utf-8` if no Content-Type is set. |
| `body`        | string                   | `""`    | Pass-through. Do **not** JSON-stringify the user's body - they want literal bytes.                                       |

State object shape (keys are exactly `d`, `s`, `h`, `b`, in that order):

```json
{"d":"<description>","s":<status>,"h":[["Header","Value"], ...],"b":"<body>"}
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
console.log("https://tools.secutils.dev/echo#" + enc);
' '{"d":"my desc","s":200,"h":[["Content-Type","text/html; charset=utf-8"]],"b":"Hello!"}'
```

Pass the state as the first argv (single-quoted JSON) so embedded quotes,
newlines, and shell metacharacters survive untouched. **Always print the full
URL** - the fragment is opaque and dropping a single character breaks decoding.

For the served-mock URL (responder inflates and replays), swap `#` for `?c=` -
same encoded blob.

## Producing the URL inline (no Node available)

The encoding is small enough to do in any language with a deflate library -
Python (`zlib.compress(..., wbits=-15)`), Rust (`flate2::write::DeflateEncoder`),
etc. Identical wire format. The byte-exact compressed output varies across
deflate implementations; that's fine - the responder uses tiny-inflate, which
accepts any valid raw-deflate stream.

## After producing

Hand the URL back verbatim in a fenced block or inline code so the user can
copy it cleanly. One sentence summary at most. Don't paraphrase the body or
abbreviate the fragment.

## Caveats

- `description` is for humans reading the share link - never include secrets;
  the fragment is visible to anyone with the URL (though never sent to the
  server when used with `#`, only with `?c=`).
- For HTML/JSON bodies, **set Content-Type explicitly** - the auto-injected
  default is `text/plain; charset=utf-8`.
- `?c=` shape is for the served mock and the fragment is browser-side only;
  use `#` when the user wants to land on the configurator UI with state
  pre-filled.
