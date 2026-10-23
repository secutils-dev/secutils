# URL state and short links

The shared fragment encoding every tool uses for reload-safe and shareable state, and the encrypted short links of `md.html` and MARS. Read before changing how a tool stores, shares, or restores state.

## URL state encoding (`encodeState` / `decodeState`)

Tools that need to remember their state across page reloads or build shareable URLs
(`jwt-debugger.html`, `certificate-decoder.html`, `saml-decoder.html`,
`echo.html`, …) must use the same compression scheme. This keeps URLs short, the calling
convention uniform, and gives any future responder script a known wire format to inflate.

### Where the encoded blob goes

**Default: the URL fragment (`#<encoded>`).**

- The fragment is never sent to the server, so it's safe for sensitive content (tokens,
  PEMs, SAML payloads). It also stays out of server / proxy logs.
- The same encoded value powers both live debounced state and the Share button - there
  is no separate share format.
- Live edits use `history.replaceState(null, '', '#' + encoded)`; the Share button
  copies the page URL with the same fragment.

**Exception: when a responder script must read the state server-side**, the share URL
uses a `?c=<encoded>` query parameter (browsers strip `#` before sending the request).
Today this applies only to `echo.html`, which still uses `#<encoded>` for its in-page
configurator and `?c=<encoded>` for the share button (the responder reads
`context.query.c` and synthesises the configured response).

### Wire format

After URL-safe base64 decoding:

```
| 4 bytes      | N bytes                                |
| ulen (LE u32)| deflate-raw of UTF-8(JSON|raw string)  |
```

The 4-byte uncompressed-length prefix is included even for browser-only tools, so any
future responder script can use a pure-JS inflater (e.g. `tiny-inflate`, like
`echo.html`'s responder) without changing the format. `tiny-inflate` requires a
pre-allocated output buffer, and the prefix lets it size that buffer in one step.

### Calling convention

The helpers are **async**, **string-in / string-out**. Callers `JSON.stringify` /
`JSON.parse` themselves at the call site when state is structured. This keeps the
helpers identical across tools regardless of payload shape.

```js
// Stash an object in the URL fragment:
history.replaceState(null, '', '#' + await encodeState(JSON.stringify(state)));

// Read it back:
const raw = await decodeState(location.hash.slice(1));
const state = raw ? JSON.parse(raw) : null;

// For tools whose state is already a string (PEM, raw SAML, etc.), skip the
// JSON.stringify / JSON.parse - pass the string directly.
```

### Canonical snippet (copy verbatim into each tool)

```js
const utf8Enc = new TextEncoder();
const utf8Dec = new TextDecoder();
const toBase64Url = (bytes) => {
    let s = '';
    for (const b of bytes) s += String.fromCharCode(b);
    return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
};
const fromBase64Url = (str) => {
    const b64 = str.replace(/-/g, '+').replace(/_/g, '/');
    const padded = b64 + '==='.slice(0, (4 - b64.length % 4) % 4);
    const bin = atob(padded);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
};
const encodeState = async (text) => {
    const bytes = utf8Enc.encode(text);
    const stream = new Blob([bytes]).stream().pipeThrough(new CompressionStream('deflate-raw'));
    const deflated = new Uint8Array(await new Response(stream).arrayBuffer());
    const out = new Uint8Array(4 + deflated.length);
    new DataView(out.buffer).setUint32(0, bytes.length, true);
    out.set(deflated, 4);
    return toBase64Url(out);
};
const decodeState = async (str) => {
    try {
        const bytes = fromBase64Url(str);
        if (bytes.length < 4) return null;
        const stream = new Blob([bytes.subarray(4)]).stream()
            .pipeThrough(new DecompressionStream('deflate-raw'));
        const inflated = new Uint8Array(await new Response(stream).arrayBuffer());
        return utf8Dec.decode(inflated);
    } catch { return null; }
};
```

### Caveats

- DEFLATE has ~10 bytes of fixed overhead, so very small payloads (under ~50 bytes) get
  slightly longer. Anything text-heavy compresses 2-10x.
- Old uncompressed share URLs (e.g. `?pem=…`, `?jwt=…&secret=…`, `?saml=…`) do **not**
  decode under this format. Migrating a tool to the canonical helpers is a clean break:
  pre-existing share links land users on the empty tool. Note this in the tool's commit
  message and move on.
- When a responder needs to read the state, mirror `echo.html`'s pattern: vendored
  `tiny-inflate` + a `ulen` cap (1 MiB is plenty) + bounds-checked source pointer to
  turn malformed input into a clean error response instead of an infinite loop.

### Short links (`md.html`, MARS)

`md.html` can also share an expiring, end-to-end-encrypted copy of the state through
its own responder KV store. The URL is `/md#s=<secret>`. Base64url never contains `=`,
so this form can't collide with `#<encoded>`. Unlike full links, the document name is
not a fragment parameter: it travels inside the ciphertext, so the URL reveals nothing.

- **Keys**: `secret` is 12 random bytes (16 base64url characters).
  `id` is the first 12 bytes of `SHA-256("id" || secret)`.
  The AES-256-GCM key is `SHA-256("key" || secret)`.
  The server sees only `id`. The key never leaves the fragment.
- **Envelope**: `base64url(IV(12) || AES-GCM(u16le(nameLen) || UTF-8 name || encodeState bytes))`.
  The name is empty when frontmatter implies it (the same rule as `stateHash()`). The canonical
  `encodeState`/`decodeState` snippet stays verbatim; bytes are bridged with
  `toBase64Url`/`fromBase64Url`. The cap is 1,398,100 characters (`SHORT_MAX_CHARS`
  in the page, `MAX_CHARS` in the script; keep them in sync), just under the 1 MiB KV
  value limit.
- **API** (the `@su:responder-script` in `md.html`, bare mount):
  - `POST ?s=<id>&ttl=<86400|604800|1209600>`, with the blob as the body and an
    `x-su-delete-token` header. It returns `201 {exp}`. Other `ttl` values fall back to
    7 days. Both `s/<id>` and `d/<id>` (SHA-256 hex of the token) are written with
    `ifAbsent`.
  - `GET ?s=<id>` returns `200 {v, exp}` or `404`.
  - `DELETE ?s=<id>` with the token header returns `204`, `403`, or `404`.
  - Errors: `400`, `405`, `413`, `429` (per-IP throttle, 20 links or 16 MiB per hour,
    tracked under `rl/<ip>/<id>.<bytes>`), `507` (KV quota).
  - Every response sets `Cache-Control: no-store` and `skipRequest: true`.
- **Client state**: `localStorage["md.shortLinks"]` maps `id` to
  `{token, exp, secret, ttl, docHash}`. Expired entries are pruned on read. Copying
  again reuses a link with the same document, name, and ttl while at least half of
  its lifetime remains. `localStorage["md.shortTtl"]` remembers the lifetime choice.
- **Tests**: `e2e/standalone/md_responder_script_smoke.spec.ts` (runs the responder script of both `md.html` and
  `mars.html`) and the short-link cases in `e2e/tools/md.spec.ts` and `mars.spec.ts`.
- **MARS** runs a copy of this responder script in its own KV, with a different plaintext ([mars.md](mars.md)).
  Keep the two scripts identical.
