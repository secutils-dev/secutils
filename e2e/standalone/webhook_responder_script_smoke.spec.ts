/**
 * Structural smoke test for the `webhook.html` `@su:responder-script` block.
 *
 * Extracts the responder script from `dev/tools/webhook.html`, runs it under
 * `node:vm` with stubbed `Deno.core` + `globalThis.secutils` (an in-memory KV
 * store and a trivial reversible "seal"), and exercises every branch of the
 * three-role responder: CAPTURE (`<mount>/<token>`), MANAGEMENT (`?t=<token>`
 * with GET/PUT/DELETE), and CONFIGURATOR (bare `<mount>` -> `null`).
 *
 * This mirrors the wire-format pairing pattern documented in
 * dev/tools/AGENTS.md -> "Responder-script smoke test", and catches a script
 * that throws/hangs or returns the wrong response shape before deploy.
 *
 * Run:
 *   cd e2e && npx playwright test --config=playwright.standalone.config.ts
 *
 * Or via Make:
 *   make e2e-standalone-test
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import * as vm from 'node:vm';

import { expect, test } from '@playwright/test';

const HTML_PATH = join(__dirname, '..', '..', 'dev', 'tools', 'webhook.html');

/** Pulls the JS between `<!-- @su:responder-script` and the closing `-->`. */
function extractResponderScript(html: string): string {
  const match = html.match(/<!--\s*@su:responder-script\s*([\s\S]*?)-->/);
  if (!match) {
    throw new Error('webhook.html is missing its @su:responder-script block');
  }
  return match[1];
}

interface ResponderResponse {
  statusCode: number;
  headers: Record<string, string>;
  body: string;
}

interface ScriptContext {
  method?: string;
  path?: string;
  query?: Record<string, string>;
  rawQuery?: string;
  headers?: Record<string, string>;
  body?: number[];
  clientAddress?: string;
}

/**
 * The responder script's own default webhook lifetime (mirrors `LIFESPAN_FALLBACK_SEC` in
 * webhook.html). The script writes the public key with this TTL, so the absolute deadline
 * holds even when the server-side `responder_kv_max_lifespan_sec` backstop is disabled
 * (its production default).
 */
const SCRIPT_LIFESPAN_SEC = 7 * 24 * 3600;

// Per-`store` side table of expiries (unix seconds, or null = no cap). Keyed by the
// store map so existing tests can keep treating `store` as a plain value map while
// `getEntry` still surfaces the server-assigned deadline.
const expiryTables = new WeakMap<Map<string, string>, Map<string, number | null>>();
function expiriesFor(store: Map<string, string>): Map<string, number | null> {
  let table = expiryTables.get(store);
  if (!table) {
    table = new Map();
    expiryTables.set(store, table);
  }
  return table;
}

/**
 * Runs the responder script once against a stubbed sandbox and resolves the
 * (possibly async) response. A single KV `store` is threaded through the calls
 * so state (registered keys, captured records) persists across invocations.
 */
async function runScript(
  script: string,
  context: ScriptContext,
  store: Map<string, string>,
  // Server-side `responder_kv_max_lifespan_sec` backstop. Defaults to 0 (disabled) - the
  // production default - so the script's own key TTL is what bounds storage. A non-zero
  // value models an operator that clamps the lifetime shorter.
  backstopSec = 0,
): Promise<ResponderResponse | null> {
  const textEncoder = new TextEncoder();
  const textDecoder = new TextDecoder();
  const expiries = expiriesFor(store);

  const kv = {
    get: async (key: string) => (store.has(key) ? store.get(key)! : null),
    getEntry: async (key: string) => ({
      value: store.has(key) ? store.get(key)! : null,
      expiresAt: store.has(key) ? (expiries.get(key) ?? null) : null,
    }),
    set: async (key: string, value: string, opts?: { ifAbsent?: boolean; ttlSec?: number }) => {
      // Mirror the real op: KV values must be unpadded base64url bytes. A raw
      // string (e.g. JSON) would throw "not valid base64url" server-side.
      if (!/^[A-Za-z0-9_-]*$/.test(value)) {
        throw new Error('KV value is not valid base64url');
      }
      if (opts?.ifAbsent && store.has(key)) {
        return;
      }
      // Mirror `effective_expires_at`: an explicit TTL sets the expiry; the backstop, when
      // enabled, caps it shorter and forces a finite expiry on any TTL-less write.
      const nowSec = Math.floor(Date.now() / 1000);
      const cap = backstopSec > 0 ? nowSec + backstopSec : null;
      const requested = opts?.ttlSec != null ? nowSec + opts.ttlSec : null;
      const effective = requested != null ? (cap != null ? Math.min(requested, cap) : requested) : cap;
      expiries.set(key, effective);
      store.set(key, value);
    },
    delete: async (key: string) => {
      expiries.delete(key);
      return store.delete(key);
    },
    list: async (opts: { prefix?: string; after?: string; limit?: number; valuesIncluded?: boolean }) => {
      const prefix = opts.prefix ?? '';
      const valuesIncluded = opts.valuesIncluded !== false;
      const entries = Array.from(store.keys())
        .filter((k) => k.startsWith(prefix) && (!opts.after || k > opts.after))
        .sort()
        .slice(0, opts.limit ?? 200)
        .map((k) => ({ key: k, createdAt: 0, value: valuesIncluded ? store.get(k)! : null }));
      const cursor = entries.length > 0 ? entries[entries.length - 1].key : null;
      return { entries, cursor, timedOut: false };
    },
    // The smoke test never blocks; `watch` behaves like an immediate `list`.
    watch: async (opts: { prefix?: string; after?: string; limit?: number }) => kv.list(opts),
  };

  const secutils = {
    kv,
    crypto: {
      // Reversible stand-in for the real sealed-box: base64url(plaintext).
      seal: async (_recipient: string, plaintext: string) =>
        Buffer.from(textEncoder.encode(plaintext)).toString('base64url'),
      sha256: async (data: string) => Buffer.from(data).toString('hex').slice(0, 16),
    },
  };

  const Deno = {
    core: {
      decode: (bytes: Uint8Array) => textDecoder.decode(bytes),
      encode: (str: string) => textEncoder.encode(str),
    },
  };

  const sandbox: Record<string, unknown> = { context, secutils, Deno, console, TextEncoder, TextDecoder };
  sandbox.globalThis = sandbox;
  vm.createContext(sandbox);

  // The script body is an IIFE expression; capture its return value.
  vm.runInContext(`globalThis.__ret = ${script}`, sandbox, { filename: 'webhook.responder.js' });
  return (await (sandbox.__ret as Promise<ResponderResponse | null>)) ?? null;
}

const TOKEN = 'Hk3rNm9bP4XyVqLs';
const PUBKEY = 'A'.repeat(96); // matches the script's /^[A-Za-z0-9_-]{80,200}$/ guard.

test.describe('webhook.html @su:responder-script', () => {
  const script = extractResponderScript(readFileSync(HTML_PATH, 'utf8'));

  test('CONFIGURATOR: bare mount returns null so the static body is served', async () => {
    const store = new Map<string, string>();
    const result = await runScript(script, { method: 'GET', path: '/webhook', query: {} }, store);
    expect(result).toBeNull();
  });

  test('CORS preflight is answered with 204 and never recorded', async () => {
    const store = new Map<string, string>();
    const result = await runScript(script, { method: 'OPTIONS', path: '/webhook', query: {} }, store);
    expect(result?.statusCode).toBe(204);
    expect(store.size).toBe(0);
  });

  test('CAPTURE on an uninitialised token is indistinguishable from 404', async () => {
    const store = new Map<string, string>();
    const result = await runScript(script, { method: 'POST', path: `/webhook/${TOKEN}`, query: {} }, store);
    expect(result?.statusCode).toBe(404);
  });

  test('a malformed token path is rejected with 404', async () => {
    const store = new Map<string, string>();
    const result = await runScript(script, { method: 'POST', path: '/webhook/!!bad!!', query: {} }, store);
    expect(result?.statusCode).toBe(404);
  });

  test('MANAGEMENT rejects a malformed ?t= token with 400', async () => {
    const store = new Map<string, string>();
    const result = await runScript(script, { method: 'GET', path: '/webhook', query: { t: '!!' } }, store);
    expect(result?.statusCode).toBe(400);
  });

  test('full lifecycle: register key -> capture -> list -> delete', async () => {
    const store = new Map<string, string>();

    // PUT registers the recipient public key (first-writer-wins).
    const reg = await runScript(
      script,
      { method: 'PUT', path: '/webhook', query: { t: TOKEN }, body: Array.from(new TextEncoder().encode(PUBKEY)) },
      store,
    );
    expect(reg?.statusCode).toBe(200);
    const regBody = JSON.parse(reg!.body);
    expect(regBody.ok).toBe(true);
    expect(regBody.mine).toBe(true);
    expect(typeof regBody.fingerprint).toBe('string');
    expect(store.has(`pk/${TOKEN}`)).toBe(true);

    // A second PUT with a different key must NOT hijack the token.
    const hijack = await runScript(
      script,
      {
        method: 'PUT',
        path: '/webhook',
        query: { t: TOKEN },
        body: Array.from(new TextEncoder().encode('B'.repeat(96))),
      },
      store,
    );
    expect(JSON.parse(hijack!.body).mine).toBe(false);
    expect(store.get(`pk/${TOKEN}`)).toBe(PUBKEY);

    // CAPTURE now seals the request and appends it to the per-token log.
    const capture = await runScript(
      script,
      {
        method: 'POST',
        path: `/webhook/${TOKEN}`,
        query: {},
        rawQuery: 'a=1',
        headers: { 'content-type': 'application/json' },
        body: Array.from(new TextEncoder().encode('{"hello":"world"}')),
        clientAddress: '203.0.113.7:5555',
      },
      store,
    );
    expect(capture?.statusCode).toBe(200);
    expect(JSON.parse(capture!.body).ok).toBe(true);
    const recordKeys = Array.from(store.keys()).filter((k) => k.startsWith(`req/${TOKEN}/`));
    expect(recordKeys.length).toBe(1);

    // The stored value is the sealed (base64url) record - decode and verify.
    const sealed = store.get(recordKeys[0])!;
    const decoded = JSON.parse(Buffer.from(sealed, 'base64url').toString('utf8'));
    expect(decoded.method).toBe('POST');
    expect(decoded.path).toBe(`/webhook/${TOKEN}`);
    expect(decoded.clientAddress).toBe('203.0.113.7:5555');

    // MANAGEMENT GET lists the sealed record.
    const list = await runScript(script, { method: 'GET', path: '/webhook', query: { t: TOKEN } }, store);
    expect(list?.statusCode).toBe(200);
    const listBody = JSON.parse(list!.body);
    expect(listBody.entries.length).toBe(1);
    expect(listBody.entries[0].value).toBe(sealed);

    // DELETE purges the records and, once drained, the key registration.
    const del = await runScript(script, { method: 'DELETE', path: '/webhook', query: { t: TOKEN } }, store);
    expect(del?.statusCode).toBe(200);
    expect(JSON.parse(del!.body).ok).toBe(true);
    expect(Array.from(store.keys()).filter((k) => k.startsWith(`req/${TOKEN}/`)).length).toBe(0);
    expect(store.has(`pk/${TOKEN}`)).toBe(false);
  });

  test('mock response config: set -> served on capture -> reset to default', async () => {
    const store = new Map<string, string>();
    const enc = (s: string): number[] => Array.from(new TextEncoder().encode(s));

    // PUT registers the key AND a custom response template in one call.
    const cfgBody = JSON.stringify({
      pk: PUBKEY,
      mock: {
        s: 202,
        h: [
          ['X-Test', '1'],
          ['Content-Type', 'application/json'],
        ],
        b: '{"queued":true}',
      },
    });
    const reg = await runScript(
      script,
      { method: 'PUT', path: '/webhook', query: { t: TOKEN }, body: enc(cfgBody) },
      store,
    );
    expect(reg?.statusCode).toBe(200);
    expect(store.has(`pk/${TOKEN}`)).toBe(true);
    expect(store.has(`cfg/${TOKEN}`)).toBe(true);

    // CAPTURE now returns the configured response, and still seals the request.
    const cap = await runScript(
      script,
      { method: 'POST', path: `/webhook/${TOKEN}`, query: {}, body: enc('hi') },
      store,
    );
    expect(cap?.statusCode).toBe(202);
    expect(cap?.headers['X-Test']).toBe('1');
    expect(cap?.headers['Content-Type']).toBe('application/json');
    expect(cap?.body).toBe('{"queued":true}');
    expect(Array.from(store.keys()).filter((k) => k.startsWith(`req/${TOKEN}/`)).length).toBe(1);

    // PUT with mock:null deletes the config (the "Reset to default" action).
    const reset = await runScript(
      script,
      { method: 'PUT', path: '/webhook', query: { t: TOKEN }, body: enc(JSON.stringify({ pk: PUBKEY, mock: null })) },
      store,
    );
    expect(reset?.statusCode).toBe(200);
    expect(store.has(`cfg/${TOKEN}`)).toBe(false);

    // CAPTURE falls back to the default 200 {"ok":true}.
    const cap2 = await runScript(
      script,
      { method: 'POST', path: `/webhook/${TOKEN}`, query: {}, body: enc('hi') },
      store,
    );
    expect(cap2?.statusCode).toBe(200);
    expect(JSON.parse(cap2!.body).ok).toBe(true);
  });

  test('absolute deadline: key, config and requests all expire together (script-owned, backstop disabled)', async () => {
    const store = new Map<string, string>();
    const expiries = expiriesFor(store);
    const enc = (s: string): number[] => Array.from(new TextEncoder().encode(s));
    const nowSec = Math.floor(Date.now() / 1000);

    // Registration returns the absolute deadline (~7 days out). The script writes the key
    // with its OWN TTL, so the deadline holds even though the server backstop is disabled -
    // the key never becomes eternal (the original unbounded-`pk` failure mode).
    const reg = await runScript(
      script,
      {
        method: 'PUT',
        path: '/webhook',
        query: { t: TOKEN },
        body: enc(JSON.stringify({ pk: PUBKEY, mock: { s: 200, h: [], b: 'ok' } })),
      },
      store,
    );
    const exp = JSON.parse(reg!.body).exp as number;
    expect(exp).toBeGreaterThan(nowSec + SCRIPT_LIFESPAN_SEC - 5);
    expect(exp).toBeLessThanOrEqual(nowSec + SCRIPT_LIFESPAN_SEC + 1);

    // A captured request inherits the same deadline rather than its own fresh window.
    await runScript(script, { method: 'POST', path: `/webhook/${TOKEN}`, query: {}, body: enc('hi') }, store);
    const reqKey = Array.from(store.keys()).find((k) => k.startsWith(`req/${TOKEN}/`))!;

    // Key, config and the captured request are all anchored to the same deadline, and none
    // is eternal.
    const pkExp = expiries.get(`pk/${TOKEN}`);
    const cfgExp = expiries.get(`cfg/${TOKEN}`);
    const reqExp = expiries.get(reqKey);
    for (const e of [pkExp, cfgExp, reqExp]) {
      expect(e).not.toBeNull();
      expect(Math.abs((e as number) - exp)).toBeLessThanOrEqual(2);
    }
  });

  test('server backstop only clamps the lifetime shorter, never longer', async () => {
    const store = new Map<string, string>();
    const expiries = expiriesFor(store);
    const enc = (s: string): number[] => Array.from(new TextEncoder().encode(s));
    const nowSec = Math.floor(Date.now() / 1000);
    const backstopSec = 3 * 24 * 3600; // operator clamps the lifetime to 3 days.

    const reg = await runScript(
      script,
      { method: 'PUT', path: '/webhook', query: { t: TOKEN }, body: enc(JSON.stringify({ pk: PUBKEY })) },
      store,
      backstopSec,
    );
    const exp = JSON.parse(reg!.body).exp as number;
    // The 3-day backstop wins over the script's 7-day default.
    expect(exp).toBeGreaterThan(nowSec + backstopSec - 5);
    expect(exp).toBeLessThanOrEqual(nowSec + backstopSec + 1);
    expect(expiries.get(`pk/${TOKEN}`) as number).toBeLessThanOrEqual(nowSec + backstopSec + 1);
  });
});
