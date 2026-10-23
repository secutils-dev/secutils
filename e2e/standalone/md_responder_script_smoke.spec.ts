/**
 * Structural smoke test for the `@su:responder-script` short-link block shared by
 * `md.html` and `mars.html` (MARS keeps its own copy, in its own responder KV).
 *
 * Extracts the responder script from each file in `dev/tools/`, runs it under `node:vm` with
 * stubbed `Deno.core` + `globalThis.secutils` (an in-memory KV store and a real SHA-256),
 * and exercises the `?s=<id>` API: POST/GET round trip, ttl whitelist, DELETE with the
 * delete token, validation errors, the per-IP throttle, and the bare mount (`null`).
 *
 * Run:
 *   make e2e-standalone-test
 */
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import * as vm from 'node:vm';

import { expect, test } from '@playwright/test';

const TOOLS_DIR = join(__dirname, '..', '..', 'dev', 'tools');

interface ResponderResponse {
  statusCode: number;
  headers: Record<string, string>;
  body: string;
  skipRequest?: boolean;
}

interface ScriptContext {
  method?: string;
  path?: string;
  query?: Record<string, string>;
  headers?: Record<string, string>;
  body?: Uint8Array;
  clientAddress?: string;
}

interface Store {
  values: Map<string, string>;
  ttls: Map<string, number | undefined>;
}

const newStore = (): Store => ({ values: new Map(), ttls: new Map() });

async function runScript(script: string, context: ScriptContext, store: Store): Promise<ResponderResponse | null> {
  const { values, ttls } = store;
  const kv = {
    get: async (key: string) => values.get(key) ?? null,
    getEntry: async (key: string) => ({
      value: values.get(key) ?? null,
      expiresAt: values.has(key) ? Math.floor(Date.now() / 1000) + (ttls.get(key) ?? 0) : null,
    }),
    set: async (key: string, value: string, opts?: { ifAbsent?: boolean; ttlSec?: number }) => {
      // Mirror the real op: values must be canonical unpadded base64url.
      if (!/^[A-Za-z0-9_-]*$/.test(value) || value.length % 4 === 1) {
        throw new Error('KV value is not valid base64url');
      }
      if (opts?.ifAbsent && values.has(key)) return;
      values.set(key, value);
      ttls.set(key, opts?.ttlSec);
    },
    delete: async (key: string) => {
      ttls.delete(key);
      return values.delete(key);
    },
    list: async (opts: { prefix?: string; limit?: number; valuesIncluded?: boolean }) => ({
      entries: Array.from(values.keys())
        .filter((k) => k.startsWith(opts.prefix ?? ''))
        .sort()
        .slice(0, opts.limit ?? 200)
        .map((key) => ({ key, createdAt: 0, value: opts.valuesIncluded === false ? null : values.get(key)! })),
      cursor: null,
      timedOut: false,
    }),
  };
  const secutils = {
    kv,
    crypto: {
      sha256: (dataB64: string) => createHash('sha256').update(Buffer.from(dataB64, 'base64url')).digest('hex'),
    },
  };
  const Deno = { core: { decode: (bytes: Uint8Array) => new TextDecoder().decode(bytes) } };
  const sandbox: Record<string, unknown> = { context: { body: new Uint8Array(), ...context }, secutils, Deno, console };
  sandbox.globalThis = sandbox;
  vm.createContext(sandbox);
  vm.runInContext(`globalThis.__ret = ${script}`, sandbox, { filename: 'md.responder.js' });
  return (await (sandbox.__ret as Promise<ResponderResponse | null>)) ?? null;
}

const ID = 'Xq3b10iSrXpQZk1u';
const TOKEN = Buffer.alloc(16, 7).toString('base64url');
const BLOB = Buffer.alloc(200, 1).toString('base64url');
const bytes = (s: string) => new TextEncoder().encode(s);
const post = (blob: string, extra: Partial<ScriptContext> = {}, query: Record<string, string> = {}): ScriptContext => ({
  method: 'POST',
  path: '/md',
  query: { s: ID, ...query },
  headers: { 'x-su-delete-token': TOKEN },
  body: bytes(blob),
  clientAddress: '203.0.113.7:0',
  ...extra,
});

for (const file of ['md.html', 'mars.html'])
  test.describe(`${file} @su:responder-script`, () => {
    const html = readFileSync(join(TOOLS_DIR, file), 'utf8');
    const script = html.match(/<!--\s*@su:responder-script\s*([\s\S]*?)-->/)![1];

    test('bare mount returns null so the static body is served', async () => {
      expect(await runScript(script, { method: 'GET', path: '/md', query: {} }, newStore())).toBeNull();
    });

    test('POST then GET round-trips the blob; every response skips request tracking', async () => {
      const store = newStore();
      const created = await runScript(script, post(BLOB), store);
      expect(created?.statusCode).toBe(201);
      expect(created?.skipRequest).toBe(true);
      expect(created?.headers['Cache-Control']).toBe('no-store');
      const exp = JSON.parse(created!.body).exp as number;
      expect(exp - Math.floor(Date.now() / 1000)).toBeGreaterThan(604800 - 5);
      expect(store.ttls.get(`s/${ID}`)).toBe(604800);
      expect(store.values.get(`d/${ID}`)).toBe(createHash('sha256').update(Buffer.alloc(16, 7)).digest('hex'));

      const read = await runScript(script, { method: 'GET', path: '/md', query: { s: ID } }, store);
      expect(read?.statusCode).toBe(200);
      expect(read?.skipRequest).toBe(true);
      expect(JSON.parse(read!.body).v).toBe(BLOB);

      const missing = await runScript(script, { method: 'GET', path: '/md', query: { s: 'A'.repeat(16) } }, store);
      expect(missing?.statusCode).toBe(404);
      expect(missing?.skipRequest).toBe(true);
    });

    test('ttl whitelist accepts 1 and 14 days and falls back to 7 days', async () => {
      for (const [ttl, expected] of [
        ['86400', 86400],
        ['1209600', 1209600],
        ['2592000', 604800],
        ['31536000', 604800],
        ['x', 604800],
      ] as const) {
        const store = newStore();
        expect((await runScript(script, post(BLOB, {}, { ttl }), store))?.statusCode).toBe(201);
        expect(store.ttls.get(`s/${ID}`)).toBe(expected);
        expect(store.ttls.get(`d/${ID}`)).toBe(expected);
      }
    });

    test('a second POST to the same id never overwrites the link or its delete token', async () => {
      const store = newStore();
      await runScript(script, post(BLOB), store);
      const other = Buffer.alloc(16, 9).toString('base64url');
      await runScript(script, post('B'.repeat(100), { headers: { 'x-su-delete-token': other } }), store);
      expect(store.values.get(`s/${ID}`)).toBe(BLOB);
      const del = await runScript(
        script,
        { method: 'DELETE', query: { s: ID }, headers: { 'x-su-delete-token': other } },
        store,
      );
      expect(del?.statusCode).toBe(403);
    });

    test('DELETE: wrong token 403, right token 204, absent 404', async () => {
      const store = newStore();
      await runScript(script, post(BLOB), store);
      const del = (token: string) =>
        runScript(script, { method: 'DELETE', query: { s: ID }, headers: { 'x-su-delete-token': token } }, store);
      expect((await del(Buffer.alloc(16, 8).toString('base64url')))?.statusCode).toBe(403);
      expect((await del('short'))?.statusCode).toBe(403);
      expect(store.values.has(`s/${ID}`)).toBe(true);
      const ok = await del(TOKEN);
      expect(ok?.statusCode).toBe(204);
      expect(ok?.skipRequest).toBe(true);
      expect(store.values.has(`s/${ID}`)).toBe(false);
      expect(store.values.has(`d/${ID}`)).toBe(false);
      expect((await del(TOKEN))?.statusCode).toBe(404);
    });

    test('validation: bad id, charset, size, token, method', async () => {
      const store = newStore();
      const status = async (ctx: ScriptContext) => (await runScript(script, ctx, store))?.statusCode;
      expect(await status({ method: 'GET', query: { s: 'short' } })).toBe(400);
      expect(await status({ method: 'GET', query: { s: 'has/slash/123456' } })).toBe(400);
      expect(await status(post('not base64url!'.repeat(10)))).toBe(400);
      expect(await status(post('A'.repeat(20)))).toBe(400);
      expect(await status(post('A'.repeat(45)))).toBe(400); // length % 4 === 1 is never valid base64url.
      expect(await status(post('A'.repeat(1398101)))).toBe(413);
      expect(await status(post(BLOB, { headers: {} }))).toBe(400);
      expect(await status({ method: 'PUT', query: { s: ID } })).toBe(405);
      expect(store.values.size).toBe(0);
    });

    test('accepts a blob at the cap, which decodes to just under the 1 MiB KV value limit', async () => {
      const store = newStore();
      const max = 'A'.repeat(1398100);
      expect(Buffer.from(max, 'base64url').length).toBeLessThanOrEqual(1048576);
      expect((await runScript(script, post(max), store))?.statusCode).toBe(201);
    });

    test('throttle: 20 links per IP per hour, then 429; other IPs unaffected', async () => {
      const store = newStore();
      for (let i = 0; i < 20; i++) {
        const id = `link${String(i).padStart(12, '0')}`;
        expect((await runScript(script, post(BLOB, { query: { s: id } }), store))?.statusCode).toBe(201);
      }
      const limited = await runScript(script, post(BLOB, { query: { s: 'overflow00000000' } }), store);
      expect(limited?.statusCode).toBe(429);
      expect(limited?.skipRequest).toBe(true);
      const other = await runScript(
        script,
        post(BLOB, { query: { s: 'overflow00000000' }, clientAddress: '198.51.100.1:0' }),
        store,
      );
      expect(other?.statusCode).toBe(201);
      expect(store.ttls.get(`rl/198.51.100.1/overflow00000000.${BLOB.length}`)).toBe(3600);
    });

    test('throttle: 16 MiB per IP per hour', async () => {
      const store = newStore();
      const big = 'A'.repeat(1398100);
      for (let i = 0; i < 12; i++) {
        const id = `big0${String(i).padStart(12, '0')}`;
        expect((await runScript(script, post(big, { query: { s: id } }), store))?.statusCode).toBe(201);
      }
      expect((await runScript(script, post(big, { query: { s: 'bigoverflow00000' } }), store))?.statusCode).toBe(429);
    });

    test('storage quota errors map to 507', async () => {
      const store = newStore();
      const set = store.values.set.bind(store.values);
      store.values.set = (key: string, value: string) => {
        if (key.startsWith('s/')) throw new Error('KV quota exceeded: total size');
        return set(key, value);
      };
      expect((await runScript(script, post(BLOB), store))?.statusCode).toBe(507);
    });
  });
