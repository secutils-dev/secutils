// Operator audit coverage: read-only execution, bounded pages, privacy, and conservative metadata hints.
import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { test } from 'node:test';
import { build } from 'esbuild';
import { runInNewContext } from 'node:vm';
import { auditStoragePage } from '../src/sync/storage-audit.ts';
import { handle, type Kv } from '../src/sync/responder.ts';
import { encode } from '../src/sync/protocol.ts';

const pack = (value: unknown) => Buffer.from(JSON.stringify(value)).toString('base64url');
const hash = (kind: string, value: string) =>
    createHash('sha256').update(`mars-storage-audit-v1:${kind}:${value}`).digest('hex');

function fixture() {
    const values = new Map<string, { value: string; expiresAt: number | null }>();
    let reads = 0;
    const write = async () => {
        throw new Error('An audit must not write or delete storage');
    };
    const kv: Kv = {
        get: async (key) => {
            reads++;
            return values.get(key)?.value ?? null;
        },
        getEntry: async (key) => {
            reads++;
            return { ...(values.get(key) ?? { value: null, expiresAt: null }) };
        },
        set: write,
        delete: write,
        list: async ({ prefix, after = '', limit, valuesIncluded }) => {
            reads++;
            assert.equal(valuesIncluded, false);
            const entries = [...values.keys()]
                .filter((key) => key.startsWith(prefix) && key > after)
                .sort()
                .slice(0, limit)
                .map((key) => ({ key, createdAt: 1700000000 }));
            return { entries, cursor: entries.at(-1)?.key ?? null };
        },
    };
    const runtime = {
        kv,
        crypto: {
            sha256: (value: string) => createHash('sha256').update(Buffer.from(value, 'base64url')).digest('hex'),
        },
    };
    const seed = (key: string, value: unknown, expiresAt: number | null = null) =>
        values.set(key, { value: pack(value), expiresAt });
    const request = async (body: Record<string, unknown>, bearer = 'operator-secret', method = 'POST') => {
        const response = await handle(
            {
                path: '/sync/storage-audit',
                method,
                body: new TextEncoder().encode(JSON.stringify(body)),
                headers: { authorization: `Bearer ${bearer}` },
                secrets: { MARS_STATS_API_KEY: 'operator-secret' },
            },
            runtime,
        );
        assert.equal(response?.skipRequest, true);
        return { status: response!.statusCode, body: JSON.parse(response!.body!) };
    };
    return { runtime, values, seed, request, reads: () => reads };
}

test('operator audit rejects ordinary credentials and malformed cursors before storage reads', async () => {
    const f = fixture();
    assert.equal((await f.request({}, 'participant-secret')).status, 403);
    assert.equal((await f.request({}, 'operator-secret', 'GET')).status, 405);
    for (const after of [{}, '../not-a-cursor', '=', 'A'.repeat(4097)]) {
        const response = await f.request({ after });
        assert.equal(response.status, 400);
        assert.equal(response.body.error, 'invalid-audit-cursor');
    }
    assert.equal(f.reads(), 0);
});

test('audit measures values without returning ciphertext, names, credentials, or raw identifiers', async () => {
    const f = fixture(),
        collection = randomUUID(),
        document = randomUUID(),
        invitation = randomUUID(),
        member = randomUUID();
    const until = Math.floor(Date.now() / 1000) + 3600;
    f.seed(`sync/v1/c/${collection}/payloads/${document}/${member}/body/checkpoint`, 'PRIVATE_DOCUMENT_CIPHERTEXT');
    f.seed(`sync/v1/c/${collection}/members/${member}`, {
        signed: {
            grant: { collection, document, expires: until, permission: 'comment', tokenHash: 'DO_NOT_RETURN_TOKEN' },
        },
    });
    f.seed(`sync/v1/invitations/${invitation}`, {
        policy: { invitation: { id: invitation, collection, document, mode: 'live', expires: until } },
        bootstrap: { ciphertext: 'DO_NOT_RETURN_KEYS' },
    });
    f.seed(`sync/v1/budget/all/00001/generation`, { collection, operation: randomUUID(), busy: true });
    f.seed('s/snapshot-id', 'PRIVATE_SNAPSHOT_CIPHERTEXT', until);
    f.seed('d/snapshot-id', 'PRIVATE_DELETE_TOKEN_HASH', until);
    const before = JSON.stringify([...f.values]);
    const response = await f.request({});
    assert.equal(response.status, 200);
    assert.equal(response.body.complete, true);
    assert.equal(response.body.rows.length, 6);
    assert.equal(
        response.body.rows.reduce((sum: number, row: { bytes: number }) => sum + row.bytes, 0),
        [...f.values.values()].reduce((sum, entry) => sum + Buffer.from(entry.value, 'base64url').length, 0),
    );
    assert.equal(
        response.body.rows.find((row: { kind: string }) => row.kind === 'sync-payload').collection,
        hash('collection', collection),
    );
    assert.equal(response.body.rows.filter((row: { liveUntil?: number }) => row.liveUntil === until).length, 2);
    for (const secret of [collection, document, invitation, member, 'PRIVATE_', 'DO_NOT_RETURN_'])
        assert.equal(JSON.stringify(response.body).includes(secret), false);
    assert.equal(JSON.stringify([...f.values]), before);
});

test('pages bound reads and do not retain large payloads in the response', async () => {
    const f = fixture(),
        collection = randomUUID(),
        document = randomUUID();
    const payload = encode(new Uint8Array(1024 * 1024));
    for (let i = 0; i < 35; i++)
        f.values.set(`sync/v1/c/${collection}/payloads/${document}/${String(i).padStart(3, '0')}`, {
            value: payload,
            expiresAt: null,
        });
    const first = await auditStoragePage({}, f.runtime);
    assert.equal(first.rows.length, 32);
    assert.equal(first.complete, false);
    assert.equal(f.reads(), 33);
    assert.ok(JSON.stringify(first).length < 12000);
    assert.equal(
        first.rows.reduce((sum, row) => sum + row.bytes, 0),
        32 * 1024 * 1024,
    );
    const next = await auditStoragePage({ after: first.cursor }, f.runtime);
    assert.equal(next.rows.length, 3);
    assert.equal(next.complete, true);
    assert.equal(f.values.size, 35);
});

test('malformed Live metadata is reported, not silently counted as Sync-only', async () => {
    const f = fixture(),
        collection = randomUUID(),
        member = randomUUID(),
        invitation = randomUUID();
    f.seed(`sync/v1/c/${collection}/members/${member}`, { malformed: 'PRIVATE' });
    f.seed(`sync/v1/invitations/${invitation}`, { malformed: 'PRIVATE' });
    const page = await auditStoragePage({}, f.runtime);
    assert.equal(page.rows.filter((row) => row.problem === 'invalid-metadata').length, 2);
    assert.equal(JSON.stringify(page).includes('PRIVATE'), false);
});

test('legacy document-scoped grants without expiry keep their Live dependency', async () => {
    const f = fixture(),
        collection = randomUUID(),
        document = randomUUID(),
        member = randomUUID();
    f.seed(`sync/v1/c/${collection}/members/${member}`, {
        signed: { grant: { collection, document, expires: 0, permission: 'view' } },
    });
    const page = await auditStoragePage({}, f.runtime);
    assert.equal(page.rows[0].liveSource, 'grant');
    assert.equal(page.rows[0].liveUntil, Number.MAX_SAFE_INTEGER);
});

test('oversized dependency metadata stays uncertain without parsing its contents', async () => {
    const f = fixture(),
        collection = randomUUID(),
        member = randomUUID();
    f.seed(`sync/v1/c/${collection}/members/${member}`, { padding: 'x'.repeat(16 * 1024) });
    const page = await auditStoragePage({}, f.runtime);
    assert.equal(page.rows[0].problem, 'oversized-metadata');
    assert.equal(page.rows[0].collection, hash('collection', collection));
    assert.equal(page.rows[0].liveUntil, undefined);
});

test('a full final page ends with an empty follow-up and missing values do not stall the cursor', async () => {
    const f = fixture();
    for (let i = 0; i < 32; i++) f.seed(`s/${String(i).padStart(2, '0')}`, 'synthetic');
    const getEntry = f.runtime.kv.getEntry;
    f.runtime.kv.getEntry = (key) =>
        key === 's/00' ? Promise.resolve({ value: null, expiresAt: null }) : getEntry(key);
    const first = await auditStoragePage({}, f.runtime);
    assert.equal(first.scanned, 32);
    assert.equal(first.rows.length, 31);
    assert.equal(first.missing, 1);
    assert.equal(first.complete, false);
    const last = await auditStoragePage({ after: first.cursor }, f.runtime);
    assert.equal(last.complete, true);
    assert.equal(last.cursor, null);
    assert.equal(last.rows.length, 0);
});

test('compiled audit works with the responder sandbox shims and no native Web Crypto', async () => {
    const compiled = await build({
        entryPoints: ['src/sync/responder.ts'],
        bundle: true,
        minify: true,
        format: 'iife',
        globalName: 'MARS_SYNC',
        target: 'es2023',
        write: false,
    });
    const api = runInNewContext(
        `(() => {
        const TextEncoder = class { encode(value) { return Deno.core.encode(value); } };
        const TextDecoder = class { decode(value) { return Deno.core.decode(value); } };
        ${compiled.outputFiles[0].text}
        return MARS_SYNC;
    })()`,
        {
            Uint8Array,
            Deno: {
                core: {
                    encode: (text: string) => new TextEncoder().encode(text),
                    decode: (bytes: Uint8Array) => new TextDecoder().decode(bytes),
                },
            },
        },
    );
    const f = fixture();
    f.seed('s/synthetic', 'PRIVATE_SNAPSHOT_CIPHERTEXT');
    const response = await api.handle(
        {
            path: '/sync/storage-audit',
            method: 'POST',
            body: new TextEncoder().encode('{}'),
            headers: { authorization: 'Bearer operator-secret' },
            params: { secrets: { MARS_STATS_API_KEY: 'operator-secret' } },
        },
        f.runtime,
    );
    assert.equal(response.statusCode, 200);
    assert.equal(JSON.parse(response.body).rows[0].kind, 'snapshot-payload');
    assert.equal(response.body.includes('PRIVATE_'), false);
});
