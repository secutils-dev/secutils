// Operator-created test scopes keep anonymous test data out of the user namespace.
// This module runs in the responder, before the Sync request handler.
// A test scope moves each KV operation of a request under `testing/v1/data/<scope>/`.
// The operator credential manages scopes. A capability token for each scope selects one.
// A failed check returns an error. It never falls back to user storage.
import { canonical, decode, encode, fail, SyncError, validHash, validId } from './protocol.ts';
import type { Context, Kv, Runtime } from './responder/types.ts';

/**
 * Registry record of one test scope. `tokenHash` is the SHA-256 of its capability token. Times are Unix seconds.
 * `removed` counts the entries that cleanup deleted.
 */
interface TestScope {
    scope: string;
    run: string;
    test: string;
    title: string;
    tokenHash: string;
    createdAt: number;
    expiresAt: number;
    state: 'active' | 'closing' | 'closed';
    removed: number;
}
/** Key prefix of all test-scope records. `scopes/` holds the registry and `data/<scope>/` the scoped data. */
const ROOT = 'testing/v1/';
const registryKey = (scope: string) => `${ROOT}scopes/${scope}`;
const dataPrefix = (scope: string) => `${ROOT}data/${scope}/`;
const now = () => Math.floor(Date.now() / 1000);
const pack = (value: unknown) => encode(new TextEncoder().encode(JSON.stringify(value)));
const unpack = (value: string): TestScope => JSON.parse(new TextDecoder().decode(decode(value)));
/** Builds a JSON response with caching and the referrer turned off. */
const response = (statusCode: number, value: unknown) => ({
    statusCode,
    body: JSON.stringify(value),
    skipRequest: true,
    headers: {
        'Content-Type': 'application/json; charset=utf-8',
        'Cache-Control': 'no-store',
        'Referrer-Policy': 'no-referrer',
    },
});
/** SHA-256 of a text through the responder runtime. */
const digestText = (runtime: Runtime, text: string) => runtime.crypto.sha256(encode(new TextEncoder().encode(text)));
/** Reads the registry record of a scope, or `null`. */
const read = async (kv: Kv, scope: string) => {
    const value = await kv.get(registryKey(scope));
    return value ? unpack(value) : null;
};

/**
 * Wraps the KV so that each key lives under the data prefix of the scope. Returned keys and cursors lose the prefix, so
 * the handler sees its normal key names. `set` reads the scope again after the write and deletes the write when the
 * scope is no longer active.
 */
function scopedKv(kv: Kv, scope: TestScope): Kv {
    const prefix = dataPrefix(scope.scope);
    const key = (value: string) => prefix + value;
    return {
        get: (value) => kv.get(key(value)),
        getEntry: (value) => kv.getEntry(key(value)),
        delete: (value) => kv.delete(key(value)),
        set: async (value, bytes, options) => {
            await kv.set(key(value), bytes, options);
            // A request that finishes after teardown must not recreate orphaned data.
            const current = await read(kv, scope.scope);
            if (!current || current.state !== 'active' || current.expiresAt <= now()) {
                await kv.delete(key(value));
                fail('test-scope-ended', 410);
            }
        },
        list: async (options) => {
            const page = await kv.list({
                ...options,
                prefix: key(options.prefix),
                after: options.after ? key(options.after) : undefined,
            });
            return {
                entries: page.entries.map((entry) => ({ ...entry, key: entry.key.slice(prefix.length) })),
                cursor: page.cursor ? page.cursor.slice(prefix.length) : null,
            };
        },
    };
}

/**
 * Entry point of test scoping. It runs before the Sync handler and returns one of three results:
 * - `{ response }`: the request was `/sync/testing`, or a scope check failed. Send this response.
 * - `{ runtime }`: the scope headers are valid. Handle the request with this runtime. Its KV stays inside the scope.
 * - `{}`: the request has no scope headers. Handle it with the normal runtime.
 *
 * `/sync/testing` needs the operator credential (`MARS_STATS_API_KEY`) as bearer token. It accepts the operations
 * `open`, `list`, and `close`. A scoped request sends `x-mars-test-scope` and `x-mars-test-token`. Request content is
 * untrusted. Each failure becomes a JSON response with a fixed code.
 */
export async function prepareTestScope(context: Context, runtime: Runtime) {
    try {
        if (context.path === '/sync/testing') {
            const secret = context.secrets?.MARS_STATS_API_KEY ?? context.params?.secrets?.MARS_STATS_API_KEY;
            const token = context.headers?.authorization?.replace(/^Bearer /, '');
            // Authenticate the operator before the body is read. The comparison uses digests of both values.
            if (!secret || !token || (await digestText(runtime, token)) !== (await digestText(runtime, secret)))
                return { response: response(403, { error: 'forbidden' }) };
            if (context.method.toUpperCase() !== 'POST')
                return { response: response(405, { error: 'method-not-allowed' }) };
            if (context.body.length > 4096) return { response: response(413, { error: 'request-too-large' }) };
            const request = JSON.parse(new TextDecoder().decode(context.body));
            if (!request || typeof request !== 'object' || Array.isArray(request)) fail('invalid-request');
            if (request.operation === 'list') {
                const prefix = `${ROOT}scopes/`;
                if (request.after && (typeof request.after !== 'string' || !request.after.startsWith(prefix)))
                    fail('invalid-cursor');
                const page = await runtime.kv.list({
                    prefix,
                    after: request.after || undefined,
                    limit: 64,
                    valuesIncluded: true,
                });
                return {
                    response: response(200, {
                        scopes: page.entries.flatMap((entry) => {
                            if (!entry.value) return [];
                            const { tokenHash: _tokenHash, ...metadata } = unpack(entry.value);
                            return [metadata];
                        }),
                        cursor: page.cursor,
                        complete: page.entries.length < 64,
                    }),
                };
            }
            if (!validId(request.scope) || !validId(request.run) || !validHash(request.test))
                fail('invalid-test-identity');
            const existing = await read(runtime.kv, request.scope);
            if (request.operation === 'open') {
                if (!validHash(request.tokenHash) || typeof request.title !== 'string' || request.title.length > 256)
                    fail('invalid-test-metadata');
                // A repeated `open` with identical metadata succeeds. Any difference is a conflict.
                if (existing) {
                    if (
                        existing.state !== 'active' ||
                        existing.run !== request.run ||
                        existing.test !== request.test ||
                        existing.tokenHash !== request.tokenHash ||
                        existing.title !== request.title ||
                        existing.expiresAt <= now()
                    )
                        fail('test-scope-conflict', 409);
                    return { response: response(200, { scope: existing.scope, expiresAt: existing.expiresAt }) };
                }
                const scope: TestScope = {
                    scope: request.scope,
                    run: request.run,
                    test: request.test,
                    title: request.title,
                    tokenHash: request.tokenHash,
                    createdAt: now(),
                    // The capability lasts one hour.
                    expiresAt: now() + 3600,
                    state: 'active',
                    removed: 0,
                };
                // `ifAbsent` selects one winner among concurrent opens. The read-back shows whether this request won.
                await runtime.kv.set(registryKey(scope.scope), pack(scope), { ifAbsent: true });
                if (canonical(await read(runtime.kv, scope.scope)) !== canonical(scope))
                    fail('test-scope-conflict', 409);
                return { response: response(201, { scope: scope.scope, expiresAt: scope.expiresAt }) };
            }
            if (request.operation !== 'close') fail('unknown-test-operation');
            // Only the run and the test that opened a scope can close it.
            if (!existing || existing.run !== request.run || existing.test !== request.test)
                return fail('test-scope-owner-mismatch', 403);
            if (existing.state === 'active') {
                existing.state = 'closing';
                await runtime.kv.set(registryKey(existing.scope), pack(existing));
            }
            // Each request deletes at most 64 entries. The operator repeats `close` until `complete` is true.
            const page = await runtime.kv.list({
                prefix: dataPrefix(existing.scope),
                limit: 64,
                valuesIncluded: false,
            });
            for (const entry of page.entries) if (await runtime.kv.delete(entry.key)) existing.removed++;
            const complete = page.entries.length < 64;
            if (complete) existing.state = 'closed';
            // The record of a closed scope stays for 14 days as an audit trail.
            await runtime.kv.set(registryKey(existing.scope), pack(existing), complete ? { ttlSec: 14 * 86400 } : {});
            return { response: response(200, { complete, removed: existing.removed }) };
        }
        const scopeId = context.headers?.['x-mars-test-scope'],
            token = context.headers?.['x-mars-test-token'];
        if (!scopeId && !token) return {};
        if (!validId(scopeId) || typeof token !== 'string' || !/^[A-Za-z0-9_-]{43}$/.test(token))
            return fail('invalid-test-scope', 403);
        const scope = await read(runtime.kv, scopeId);
        if (!scope || scope.state !== 'active' || scope.expiresAt <= now()) return fail('test-scope-ended', 410);
        if ((await digestText(runtime, token)) !== scope.tokenHash) fail('forbidden-test-scope', 403);
        return { runtime: { ...runtime, kv: scopedKv(runtime.kv, scope) } };
    } catch (error) {
        return {
            response: response(error instanceof SyncError ? error.status : error instanceof SyntaxError ? 400 : 503, {
                error:
                    error instanceof SyncError
                        ? error.code
                        : error instanceof SyntaxError
                          ? 'invalid-json'
                          : 'test-scope-unavailable',
            }),
        };
    }
}
