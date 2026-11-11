import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import * as vm from 'node:vm';
import { configureMarsStatsScript, marsStatsCollectorSettings, MARS_STATS_SECRETS } from '../dev/tools/mars-stats/config.ts';
import { inlineSharedAssets } from '../dev/tools/shared/assets.ts';
import { ICONS } from '../dev/tools/shared/icons.js';

const token = Buffer.alloc(32, 7).toString('base64url');
const id = '01a16670-bd06-7359-8649-cebe473a8d2d';
const apiKey = 'su_ak_account-secret-never-returned';
const env = { SECUTILS_HTML_APP_RESPONDER_ID_MARS: id, SECUTILS_MARS_STATS_VIEW_TOKEN: token, SECUTILS_HTML_APP_API_DOMAIN: 'https://secutils.dev' };
const source = readFileSync(new URL('../dev/tools/mars-stats/index.html', import.meta.url), 'utf8');
const script = configureMarsStatsScript(source.match(/<!--\s*@su:responder-script\s*([\s\S]*?)-->/)![1], env);
const storage = { entries: 194, totalBytes: 123129, maxEntries: 100000, maxTotalBytes: 1073741824, oldestCreatedAt: 1700000000, nextExpiresAt: 1900000000 };
const summary = { items: [{ id, settingsSize: 268352, enabled: true, settings: { requestsToTrack: 30 }, stats: { requestCount: 30, lastRequestedAt: 1793000000 }, updatedAt: 1793000100 }] };
type Entry = { value: string; expiresAt: number | null };

function harness() {
    const values = new Map<string, Entry>();
    const calls: { url: string; headers: Record<string, string> }[] = [];
    let now = 1793200000;
    let status = 200;
    let rawStorage: unknown = storage;
    let failStorage = false;
    let proxyError: string | null = null;
    let cacheWritesFail = false;
    let sync: Record<string, unknown> | null = null;
    const kv = {
        get: async (key: string) => {
            const entry = values.get(key);
            return entry && (entry.expiresAt == null || entry.expiresAt > now) ? entry.value : null;
        },
        set: async (key: string, value: string, options: { ifAbsent?: boolean; ttlSec?: number } = {}) => {
            if (failStorage) throw new Error(apiKey);
            if (cacheWritesFail && !key.startsWith('h/')) throw new Error(apiKey);
            assert.equal(Buffer.from(value, 'base64url').toString('base64url'), value);
            if (options.ifAbsent && await kv.get(key)) return;
            values.set(key, { value, expiresAt: options.ttlSec ? now + options.ttlSec : null });
        },
        list: async (options: { prefix: string; after: string; limit: number }) => {
            const entries = [];
            for (const key of [...values.keys()].sort()) {
                if (!key.startsWith(options.prefix) || key <= options.after) continue;
                const value = await kv.get(key);
                if (value != null) entries.push({ key, value, createdAt: now });
                if (entries.length >= options.limit) break;
            }
            return { entries, cursor: entries.at(-1)?.key ?? null };
        },
    };
    const run = async (action?: string, options: { method?: string; token?: string; query?: Record<string, string>; secrets?: Record<string, string> } = {}) => {
        const context = { method: options.method ?? 'GET', query: { ...(action ? { action } : {}), ...options.query },
            headers: { authorization: `Bearer ${options.token ?? token}` }, secrets: options.secrets ?? { MARS_STATS_API_KEY: apiKey } };
        const result = await vm.runInNewContext(script, {
            context, Date: class extends Date { static now() { return now * 1000; } },
            secutils: { kv, crypto: { sha256: async (data: string) => createHash('sha256').update(Buffer.from(data, 'base64url')).digest('hex') } },
            Deno: { core: { encode: (text: string) => new TextEncoder().encode(text), decode: (bytes: Uint8Array) => new TextDecoder().decode(bytes), ops: {
                op_proxy_request: async (request: { url: string; headers: Record<string, string> }) => {
                    calls.push(request);
                    if (proxyError) throw new Error(proxyError);
                    if (request.url.startsWith('https://mars.secutils.dev/sync/')) {
                        assert.equal(request.headers.Authorization, `Bearer ${apiKey}`);
                        return { statusCode: sync ? 200 : 503, body: [...Buffer.from(JSON.stringify(request.url.endsWith('/telemetry') ? sync ?? { error: 'unavailable' } : { complete: true, cursor: null }))] };
                    }
                    assert.ok(request.url.startsWith('https://secutils.dev/api/webhooks/responders'));
                    assert.equal(request.headers.Authorization, `Bearer ${apiKey}`);
                    return { statusCode: status, body: [...Buffer.from(JSON.stringify(status !== 200 ? { leaked: apiKey } : request.url.endsWith('/_storage') ? rawStorage : summary))] };
                },
            } } },
        });
        if (result) {
            assert.equal(result.skipRequest, true);
            assert.equal(result.headers['Cache-Control'], 'no-store');
            assert.ok(!result.body.includes(apiKey));
            return { ...result, data: JSON.parse(result.body) };
        }
        return null;
    };
    return { values, calls, run, advance: (sec: number) => { now += sec; }, setStatus: (n: number) => { status = n; }, setStorage: (value: unknown) => { rawStorage = value; }, failWrites: () => { failStorage = true; },
        setProxyError: (message: string) => { proxyError = message; }, failCacheWrites: () => { cacheWritesFail = true; }, setSync: (value: Record<string, unknown>) => { sync = value; } };
}

test('static fallback, authenticated reads, and no credential exposure', async () => {
    const h = harness();
    assert.equal(await h.run(), null);
    assert.equal(h.calls.length, 0);
    const current = await h.run('stats');
    assert.equal(current.statusCode, 200);
    assert.deepEqual(current.data.storage, storage);
    assert.equal(current.data.responder.settingsSize, 268352);
    assert.equal(current.data.intervalSec, 1800);
    assert.equal(h.calls.length, 3);
    await h.run('stats');
    assert.equal(h.calls.length, 3, 'reads within 60 seconds use the cache');
    const history = await h.run('history');
    assert.equal(history.data.intervalSec, 1800);
    assert.equal(history.data.points.length, 1);
    assert.equal(history.data.points[0][2], storage.totalBytes);
    assert.ok([...h.values.values()].every((e) => !Buffer.from(e.value, 'base64url').toString().includes(apiKey)));
});
test('sync samples report allocation separately and do not expose unknown upstream fields', async () => {
    const h = harness();
    h.setSync({ reservedBytes: 65536, maximumReservedBytes: 943718400, collectionsWithReservations: 1, reservedSlots: 1, unitBytes: 65536, privateDocument: 'must not escape' });
    const sample = await h.run('sample', { method: 'POST', token: apiKey });
    assert.equal(sample.statusCode, 200);
    const current = await h.run('stats');
    assert.equal(current.data.sync.reservedBytes, 65536);
    assert.equal(current.body.includes('must not escape'), false);
    const history = await h.run('history');
    assert.equal(history.data.points[0][11], 65536);
    assert.equal(history.data.columns[11], 'syncReservedBytes');
    assert.ok(h.calls.some((call) => call.url.endsWith('/sync/maintenance')));
});

test('authorization gates reads and collection before any proxy or KV work', async () => {
    const h = harness();
    for (const bad of ['', 'wrong', Buffer.alloc(32, 8).toString('base64url'), apiKey]) {
        assert.equal((await h.run('stats', { token: bad })).statusCode, 401);
        assert.equal((await h.run('history', { token: bad })).statusCode, 401);
    }
    assert.equal((await h.run('sample', { method: 'POST' })).statusCode, 401);
    assert.equal((await h.run('sample', { method: 'POST', token: apiKey })).statusCode, 200);
    assert.equal(h.calls.length, 3);
});

test('malformed requests return clean errors', async () => {
    const h = harness();
    assert.equal((await h.run('unknown')).statusCode, 400);
    assert.equal((await h.run('stats', { method: 'DELETE' })).statusCode, 405);
    assert.equal((await h.run('sample')).statusCode, 405);
    for (const query of [{ since: 'NaN' }, { since: '-1' }, { after: 's/private-link' }]) {
        assert.equal((await h.run('history', { query })).statusCode, 400);
    }
    assert.equal(h.calls.length, 0);
});

test('samples are immutable per bucket and expire after 30 days', async () => {
    const h = harness();
    await h.run('sample', { method: 'POST', token: apiKey });
    const key = [...h.values.keys()].find((k) => k.startsWith('h/'))!;
    const original = h.values.get(key)!;
    assert.equal(original.expiresAt, 1793200000 + 2592000);
    assert.equal((await h.run('sample', { method: 'POST', token: apiKey })).data.sampled, false);
    assert.equal(h.calls.length, 3);
    h.advance(61); h.setStorage({ ...storage, entries: 200 });
    await h.run('stats');
    assert.equal(h.values.get(key)!.value, original.value);
    h.advance(1800); await h.run('sample', { method: 'POST', token: apiKey });
    assert.equal((await h.run('history')).data.points.length, 2);
    h.advance(2592000);
    assert.equal((await h.run('history')).data.points.length, 0);
});

test('old five-minute samples and cached metadata survive the interval change', async () => {
    const h = harness();
    const legacyKey = 'h/001793199900';
    const legacyValue = Buffer.from(JSON.stringify([1793199900, 194, 123129])).toString('base64url');
    h.values.set(legacyKey, { value: legacyValue, expiresAt: 1793200000 + 2592000 });
    h.values.set('latest', { value: Buffer.from(JSON.stringify({ intervalSec: 300 })).toString('base64url'), expiresAt: 1793200060 });
    assert.equal((await h.run('stats')).data.intervalSec, 1800);
    assert.equal(h.calls.length, 3, 'old interval metadata must not be served from the cache');
    assert.equal(h.values.get(legacyKey)!.value, legacyValue);
    assert.equal((await h.run('history')).data.points.length, 2);
});

test('history paginates and only reads telemetry-prefixed entries', async () => {
    const h = harness();
    h.values.set('unrelated/private', { value: 'invalid', expiresAt: null });
    for (let i = 0; i < 1001; i++) {
        const at = 1792800000 + i * 300;
        h.values.set(`h/${String(at).padStart(12, '0')}`, { value: Buffer.from(JSON.stringify([at, i, i * 20])).toString('base64url'), expiresAt: null });
    }
    const first = await h.run('history');
    assert.equal(first.data.points.length, 1000);
    const second = await h.run('history', { query: { after: first.data.cursor } });
    assert.equal(second.data.points.length, 1);
    assert.equal(second.data.cursor, null);
    assert.equal(h.calls.length, 0);
});

test('upstream failures, malformed schemas, missing secrets, and quota failures stay private', async () => {
    for (const status of [401, 403, 404, 500]) {
        const h = harness(); h.setStatus(status);
        assert.equal((await h.run('stats')).statusCode, 502);
        assert.equal(h.values.size, 0);
    }
    const invalid = harness(); invalid.setStorage({ ...storage, totalBytes: '1000' });
    assert.equal((await invalid.run('stats')).statusCode, 502);
    assert.equal(invalid.values.size, 0);
    const missing = harness(); assert.equal((await missing.run('stats', { secrets: {} })).statusCode, 503);
    const full = harness(); full.failWrites(); assert.equal((await full.run('stats')).statusCode, 507);
});

test('network and validation errors are classified without disclosing raw error text', async () => {
    for (const [message, code] of [
        [`URL not allowed (non-public address): ${apiKey}`, 'upstream-url-validation'],
        [`Upstream request timed out: ${apiKey}`, 'upstream-timeout'],
        [`Failed to connect to upstream: ${apiKey}`, 'upstream-network'],
    ]) {
        const h = harness(); h.setProxyError(message);
        const response = await h.run('sample', { method: 'POST', token: apiKey });
        assert.equal(response.statusCode, 502);
        assert.equal(response.data.code, code);
        assert.equal(response.data.stage, 'storage-api');
        assert.equal(response.data.retryable, true);
        assert.equal(h.values.size, 0);
    }
});

test('only dashboard reads fall back to explicitly stale last-good data', async () => {
    const h = harness();
    const original = await h.run('stats');
    h.advance(1800); h.setStatus(503);
    const stale = await h.run('stats');
    assert.equal(stale.statusCode, 200);
    assert.equal(stale.data.stale, true);
    assert.equal(stale.data.at, original.data.at, 'a cached snapshot must keep its real observation time');
    assert.equal(stale.data.failure.code, 'upstream-http-503');
    const collected = await h.run('sample', { method: 'POST', token: apiKey });
    assert.equal(collected.statusCode, 502);
    assert.equal(collected.data.code, 'upstream-http-503');
    assert.equal((await h.run('history')).data.points.length, 1, 'failures must not create fake history');
    assert.equal((await h.run('stats', { token: 'invalid' })).statusCode, 401);
    h.advance(2592000);
    assert.equal((await h.run('stats')).statusCode, 502, 'expired last-good snapshots are not served');
});

test('optional cache write failures do not discard a successful sample', async () => {
    const h = harness(); h.failCacheWrites();
    assert.equal((await h.run('sample', { method: 'POST', token: apiKey })).statusCode, 200);
    assert.equal((await h.run('history')).data.points.length, 1);
});

test('empty storage keeps nullable timestamps and zero entry counts', async () => {
    const h = harness(); h.setStorage({ ...storage, entries: 0, totalBytes: 0, oldestCreatedAt: null, nextExpiresAt: null });
    assert.equal((await h.run('stats')).data.storage.oldestCreatedAt, null);
});

test('deployment substitutes only server settings and reuses real MARS tokens and glyphs', () => {
    assert.ok(!script.includes(token));
    assert.ok(script.includes(createHash('sha256').update(Buffer.from(token, 'base64url')).digest('hex')));
    assert.ok(script.includes(id));
    assert.throws(() => configureMarsStatsScript(script, { ...env, SECUTILS_MARS_STATS_VIEW_TOKEN: 'short' }));
    assert.throws(() => configureMarsStatsScript(script, { ...env, SECUTILS_HTML_APP_API_DOMAIN: 'https://secutils.dev@evil.test/path' }));
    const html = inlineSharedAssets(source);
    assert.match(html, /<style data-su-style="mars">/);
    assert.ok(html.includes(readFileSync(new URL('../dev/tools/shared/themes/mars.css', import.meta.url), 'utf8')));
    assert.doesNotMatch(html, /href="\.\.\/(shared|mars)\//);
    for (const [, name] of source.matchAll(/data-icon="([^"]+)"/g)) assert.ok(name in ICONS, `Missing glyph: ${name}`);
    assert.ok(Buffer.byteLength(script) < 12000, 'keep the responder script small');
});

test('the collector injects its selected secret at runtime and retains only one revision', () => {
    const settings = marsStatsCollectorSettings();
    assert.deepEqual(settings.secrets, MARS_STATS_SECRETS);
    assert.equal(settings.config.revisions, 1);
    assert.equal(settings.config.job.schedule, '0 */30 * * * *');
    assert.deepEqual(settings.config.job.retryStrategy, { type: 'exponential', initialInterval: 60000, multiplier: 2, maxInterval: 300000, maxAttempts: 3 });
    assert.equal(settings.notifications, false);
    assert.ok(!JSON.stringify(settings).includes(apiKey));
    const result = vm.runInNewContext(settings.target.configurator, { context: { params: { secrets: { MARS_STATS_API_KEY: apiKey } }, requests: [{ url: settings.target.url }] } });
    assert.equal(result.requests[0].headers.Authorization, `Bearer ${apiKey}`);
});
