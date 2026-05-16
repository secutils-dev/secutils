---
name: secutils-deno-runtime
description: >-
  Reference for the restricted Deno runtime that hosts Secutils.dev
  responder scripts and API tracker configurator/extractor scripts.
  Documents what is and is not available inside the sandbox, the
  `context` global shape per script kind, the `Deno.core` utility
  namespace (encode/decode, binary string helpers, base64 building
  blocks, type checks), the body auto-conversion table, and the
  `Deno.core.ops.op_proxy_request()` API used to safely forward HTTP
  requests upstream. Load this skill whenever an agent is writing or
  debugging JavaScript that will be pasted into a Secutils responder
  or tracker script field, so it generates code that actually runs
  inside the sandbox instead of code that relies on `fetch`, `btoa`,
  `setTimeout`, or other globals that are not exposed.
---

# Secutils.dev: Deno Sandbox Runtime (reference)

This skill is **reference material only**. It does not invoke any
Secutils.dev HTTP endpoint. Use it as a companion skill to
`secutils-webhooks`, `secutils-web-scraping-api`,
`secutils-web-scraping-page`, and `secutils-user-scripts` whenever an
agent is producing the JS body that those endpoints accept.

Full human-readable reference:
<https://secutils.dev/docs/guides/platform/deno_runtime>. The complete
list of `Deno.core` members exposed in the sandbox is published live at
<https://demo.webhooks.dev.secutils.dev/deno-apis>.

## Where the sandbox runs

| Surface                     | Wrapper                                                                                                                   | `context` global shape                                                                 |
|-----------------------------|---------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------|
| Responder script (webhooks) | Wrapped with an outer IIFE; the script body may begin directly with statements or with its own `(async () => { ... })()`. | `{ clientAddress?, method, headers, path, rawQuery?, query, body: number[], secrets }` |
| API tracker configurator    | Must be a self-invoking IIFE: `(() => { ... })();`                                                                        | `{ requests, params: { secrets } }`; return `{ requests }`                             |
| API tracker extractor       | Must be a self-invoking IIFE                                                                                              | `{ responses, params: { secrets, previousContent? } }`; return `{ body }`              |
| Page tracker extractor      | Must export `async function execute(page, context)`                                                                       | `context = { params: { secrets }, previousContent? }`; `page` is a Playwright Page     |

The IIFE-wrapped sandboxes accept a returned value that auto-converts
to bytes (see Body auto-conversion below). The page tracker's
`execute()` may return any JSON-serialisable value or a `Uint8Array`.

## What is NOT available inside the sandbox

The sandbox is intentionally locked down. Do not generate code that
relies on the following:

- `fetch`, `XMLHttpRequest`, WebSocket, EventSource. For outbound HTTP
  from a responder script, use `Deno.core.ops.op_proxy_request()`
  (below). For API trackers, declare the requests in
  `target.requests` and let the runtime issue them; the configurator
  can only mutate them, not perform them.
- `btoa`, `atob`. See the base64 recipe below for pure-JS replacements.
- `setTimeout`, `setInterval`, `queueMicrotask`. Use
  `await Promise.resolve()` for microtask yields.
- File system, child process, OS, FFI.
- `WebCrypto` (`crypto.subtle`). Cryptographic primitives must be
  implemented in pure JS or done outside the sandbox (e.g. the
  certificate template endpoint or the JWT debugger tool).

What IS available: every standard ECMAScript built-in (`JSON`, `Math`,
`Date`, `Map`, `Set`, `Promise`, `TextEncoder`, `TextDecoder`,
`Array`, typed arrays, `Intl`, regex), the `Deno.core` namespace
documented below, and the script-specific `context` global.

## `Deno.core` namespace

| Member                                                   | Purpose                                                                                                  |
|----------------------------------------------------------|----------------------------------------------------------------------------------------------------------|
| `Deno.core.encode(text: string): Uint8Array`             | UTF-8 encode a string.                                                                                   |
| `Deno.core.decode(buf: Uint8Array): string`              | UTF-8 decode a buffer.                                                                                   |
| `Deno.core.encodeBinaryString(buf: Uint8Array): string`  | Latin-1 binary string (one byte per char). Useful as input to a pure-JS base64 encoder.                  |
| `Deno.core.ops.op_proxy_request(req): Promise<Response>` | Forward an HTTP request through the Secutils edge with built-in SSRF protection. Responder scripts only. |

The full list of `Deno.core` symbols is enumerable at runtime:

```javascript
(async () => ({ body: JSON.stringify(Object.keys(Deno.core).sort()) }))();
```

Drop that script into a responder to print the live API surface.

## `context` shape per script kind

### Responder script

```typescript
interface Context {
  clientAddress?: string;          // real client IP, x-forwarded-for-resolved
  method: string;                  // HTTP method of the incoming request
  headers: Record<string, string>; // includes `x-forwarded-*` and `x-real-ip`
  path: string;                    // request path (without query)
  rawQuery?: string;               // query string without leading `?`
  query: Record<string, string>;   // parsed query
  body: number[];                  // raw request body bytes
  secrets: Record<string, string>; // decrypted secrets the responder may access
}
```

### API tracker configurator

```typescript
interface ConfiguratorContext {
  requests: Array<{ url: string; method: string; headers: Record<string, string>; body?: string; acceptInvalidCertificates?: boolean }>;
  params: { secrets: Record<string, string> };
}
// Return shape:
interface ConfiguratorResult { requests: ConfiguratorContext['requests'] }
```

### API tracker extractor

```typescript
interface ExtractorContext {
  tags: string[];
  previousContent?: { original: unknown };
  responses?: Array<{ status: number; headers: Record<string, string>; body: number[] }>;
  params?: { secrets?: Record<string, string> };
}
// Return shape:
interface ExtractorResult { body?: Uint8Array | string | object | number | boolean }
```

### Page tracker extractor

```typescript
async function execute(page: import('playwright').Page, context: {
  params: { secrets: Record<string, string> };
  previousContent?: { original: unknown };
}): Promise<unknown | Uint8Array>;
```

## Body auto-conversion

Every sandbox that accepts a `body` (responder `ScriptResult.body`, API
tracker extractor `ExtractorResult.body`, page tracker `execute()`
return value) accepts these input types and converts to bytes:

| Input type            | Converted to                                                           |
|-----------------------|------------------------------------------------------------------------|
| `Uint8Array`          | as-is                                                                  |
| `string`              | UTF-8 bytes (`Deno.core.encode`)                                       |
| plain object or array | `JSON.stringify` then UTF-8 bytes                                      |
| `number` or `boolean` | `String(x)` then UTF-8 bytes                                           |
| `null` or `undefined` | empty body (and, for responders, the static default body is preserved) |

For binary responses (PNG, PDF, etc.) build the `Uint8Array`
explicitly; the auto-converter never tries to interpret string content
as base64.

## Base64 without `btoa` / `atob`

Encode (replaces `btoa`):

```javascript
function toBase64(input) {
  const CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
  const bytes = typeof input === 'string' ? Deno.core.encode(input) : input;
  let result = '';
  for (let i = 0; i < bytes.length; i += 3) {
    const a = bytes[i], b = bytes[i + 1] ?? 0, c = bytes[i + 2] ?? 0;
    result += CHARS[a >> 2] + CHARS[((a & 3) << 4) | (b >> 4)];
    result += i + 1 < bytes.length ? CHARS[((b & 15) << 2) | (c >> 6)] : '=';
    result += i + 2 < bytes.length ? CHARS[c & 63] : '=';
  }
  return result;
}
```

Decode (replaces `atob`):

```javascript
function fromBase64(b64) {
  const CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
  const s = b64.replace(/=+$/, '');
  const out = [];
  for (let i = 0; i < s.length; i += 4) {
    const a = CHARS.indexOf(s[i]);
    const b = CHARS.indexOf(s[i + 1]);
    const c = CHARS.indexOf(s[i + 2]);
    const d = CHARS.indexOf(s[i + 3]);
    out.push((a << 2) | (b >> 4));
    if (c >= 0) out.push(((b & 15) << 4) | (c >> 2));
    if (d >= 0) out.push(((c & 3) << 6) | d);
  }
  return Deno.core.decode(new Uint8Array(out));
}
```

For URL-safe base64 (used by JWT, etc.), replace the alphabet's last
two chars with `-_` and strip trailing `=`.

## `op_proxy_request` (responder scripts only)

Forward an HTTP request through the Secutils edge proxy. Built-in
SSRF protection blocks RFC 1918 ranges, link-local addresses, and
loopback.

```typescript
interface ProxyRequest {
  url: string;
  method?: string;                         // default GET
  headers?: Record<string, string>;
  body?: number[] | Uint8Array;
  insecure?: boolean;                      // accept self-signed TLS
  timeout?: number;                        // milliseconds, default 30000
}

interface ProxyResponse {
  statusCode: number;
  headers: Record<string, string>;
  body: number[];                          // automatically decompressed (gzip/deflate/brotli)
}

const resp = await Deno.core.ops.op_proxy_request({
  url: 'https://api.example.com/widgets',
  method: 'GET',
  headers: { authorization: `Bearer ${context.secrets.UPSTREAM_TOKEN}` },
  timeout: 5000,
});
```

Errors (non-2xx are NOT errors; only network/timeout/SSRF failures
throw) bubble out as exceptions; the responder script can catch them
and return a custom error response.

## Recipes catalogue (where to find them)

- Pure proxy / response-mutating proxy / conditional proxy:
  <https://secutils.dev/docs/guides/webhooks#proxy-requests-to-an-upstream-service-mitm>
- HTTP Basic auth gate:
  <https://secutils.dev/docs/guides/webhooks#protect-a-responder-with-http-basic-auth>
- Cookie-session login form:
  <https://secutils.dev/docs/guides/webhooks#protect-a-responder-with-a-login-form-cookie-session>
- Selective tracking / response tracking:
  <https://secutils.dev/docs/guides/webhooks#selectively-track-requests>
- PNG generation (binary body):
  <https://secutils.dev/docs/guides/webhooks#generate-images-and-other-binary-content>

## See also

- Human-readable reference: <https://secutils.dev/docs/guides/platform/deno_runtime>
- Consumers: `secutils-webhooks`, `secutils-web-scraping-page`,
  `secutils-web-scraping-api`, `secutils-user-scripts`
- Live API surface: <https://demo.webhooks.dev.secutils.dev/deno-apis>
