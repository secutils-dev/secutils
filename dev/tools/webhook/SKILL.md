---
name: webhook-inspector
description: >-
  Get an ephemeral, end-to-end-encrypted webhook URL on Secutils.dev and inspect
  the HTTP requests it captures. Point any service at
  https://tools.secutils.dev/webhook/<token> (webhooks, Report-To/CSP reporting,
  OAuth callbacks, IPN/payment notifications, etc.), then open
  https://tools.secutils.dev/webhook#<session> to watch requests arrive live.
  Trigger when the user asks for a "webhook URL", "request bin", "request
  inspector", "catch a webhook", "CSP report endpoint", "test callback URL", or
  anything that names secutils.dev/webhook.
---

# Ephemeral webhook inspector (Secutils.dev)

Create a unique, throwaway URL that captures every HTTP request sent to it, then
view those requests (method, path, query, headers, body, client address) in a
live-updating grid. No account, no signup.

Two URL shapes are involved:

| URL                                            | Role                                                                                           |
|------------------------------------------------|------------------------------------------------------------------------------------------------|
| `https://tools.secutils.dev/webhook/<token>`   | **Capture URL** - give this to the service you want to inspect. Any HTTP method works.         |
| `https://tools.secutils.dev/webhook#<session>` | **Inspector URL** - opens the UI with the matching decryption key and shows captured requests. |

## End-to-end encryption (why there are two URLs)

When a webhook is created, the browser generates an **ECDH P-256** key pair. Only
the public key is registered with the server. Each captured request is sealed on
the server (ECDH → HKDF-SHA256 → AES-256-GCM, libsodium-style sealed box) and
stored as ciphertext. The private key lives only in the browser, encoded into
the inspector URL's fragment (`#…`, never sent to the server) and in
`localStorage`. Whoever holds the inspector URL can decrypt; the server cannot.

Each webhook is **ephemeral with a single absolute lifespan of 7 days** from creation.
At the deadline the webhook URL, its response template, **and** every captured request
are deleted together - the inspector shows an "expired" state once that happens. The
deadline is fixed when the key is first registered (the key is stored with that TTL, so it
can never outlive the webhook) and surfaced to the UI as `exp` (a unix timestamp in
seconds). An operator can shorten the lifespan server-side via `responder_kv_max_lifespan_sec`,
but never extend it. For permanent webhooks, create a free Secutils.dev account.

## How to get a webhook (human flow)

1. Open `https://tools.secutils.dev/webhook` - a fresh webhook is minted on first
   visit (or click **+ New**).
2. Copy the **Webhook URL** (`…/webhook/<token>`) and point your service at it.
3. Click **Live** and watch requests stream in.
   Optionally open **Response** to customize the status code, headers, and body
   returned to callers (handy for handshake/verification callbacks). The response
   template is stored unencrypted server-side; only captured requests are encrypted.
4. To reopen the same webhook elsewhere, share the full inspector URL **including
   the `#…` fragment** - that fragment carries the decryption key.
5. A badge next to the URL shows when the webhook expires; hover it for the exact
   deletion time. Use **Clone** to copy the current label and response template into
   a brand-new webhook with a fresh URL, key, and 7-day deadline (captured requests
   are intentionally **not** copied - the clone has a new key and cannot read them).

`https://tools.secutils.dev/webhook#new` always mints a fresh webhook,
bypassing any locally stored sessions.

## REST surface (for automation)

All management calls target the bare mount with `?t=<token>`; capture targets the
sub-path. The token must match `^[A-Za-z0-9_-]{8,64}$`.

| Method   | URL                                          | Purpose                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
|----------|----------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `ANY`    | `/webhook/<token>`                           | Capture a request (sealed + stored). Returns the token's configured mock response, or `{"ok":true}` by default. `404` if the token's key was never registered.                                                                                                                                                                                                                                                                                                                                                                         |
| `PUT`    | `/webhook?t=<token>`                         | Register the recipient public key and/or set the mock response. Body = base64url of the raw 65-byte SEC1 P-256 public key, or JSON `{"pk":"…","mock":{…}}`. The key is first-writer-wins. `mock` is `{s,h,b}` (status, header pairs, body, same shape as the Echo tool) to set the response template, `null` to delete it, or omitted to leave it unchanged. Returns `{ok, fingerprint, mine, exp}` (`exp` = the webhook's absolute deadline, unix seconds). The key and config inherit that deadline so they expire with the webhook. |
| `GET`    | `/webhook?t=<token>[&after=<cursor>]`        | List sealed records (oldest→newest). Returns `{entries:[{key,createdAt,value}], cursor, timedOut}`.                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `GET`    | `/webhook?t=<token>&live=1[&after=<cursor>]` | Same as list, but long-polls (up to ~25 s) until new records arrive.                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `DELETE` | `/webhook?t=<token>[&keep=1]`                | Delete a page of records. Repeat while `remaining` is `true`. Without `keep=1`, also drops the key once drained.                                                                                                                                                                                                                                                                                                                                                                                                                       |

`value` and the registered public key are base64url. A sealed `value` decodes to:

```
| 65 bytes ephemeral SEC1 public key | 12 bytes AES-GCM IV | ciphertext + 16-byte GCM tag |
```

Decryption (browser `crypto.subtle`): `ECDH(privateKey, ephemeralPub)` → 32-byte
X coordinate → `HKDF-SHA256(ikm=X, salt=yourPublicKeyRaw, info=ephemeralPubRaw)`
→ AES-256 key → `AES-GCM(iv)` over `ciphertext+tag`. The plaintext is JSON:
`{at, method, path, query, headers, clientAddress, bodyB64}` (`bodyB64` is
base64url of the raw request body).

## Caveats

- The inspector URL fragment **is** the decryption key. Anyone with the full URL
  can read captured requests. Treat it like a secret; never paste it where the
  fragment would be logged server-side.
- Lose the fragment (and the `localStorage` copy) and the captured data is
  unrecoverable by design. There is no server-side key escrow.
- Capture returns `404` until the public key is registered, so always register
  before sharing the capture URL (the UI does this automatically on load).
- The whole webhook is ephemeral (single 7-day absolute lifespan: URL, response
  template, and captured requests all expire together). Capture also returns `404`
  once the webhook has expired and been swept. For durable, authenticated request
  history and permanent webhooks, create a free Secutils.dev account / responder instead.
