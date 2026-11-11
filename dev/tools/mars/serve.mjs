// Local dev server: serves dist/mars.html (from `npm run preview`) at / and the skill at
// /llms.txt, like the MARS host, and runs the page's `@su:responder-script` against an
// in-memory KV, so short links work offline.
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import * as vm from 'node:vm';
import { inlineMarsSync } from './sync-assets.ts';

// Port of the local server. The default is 7373.
const PORT = Number(process.env.PORT) || 7373;
// The snapshot responder script from the comment at the top of `index.html`, with the link
// preview image filled in as deployment does.
const legacyScript = readFileSync('index.html', 'utf8')
    .match(/<!--\s*@su:responder-script\s*([\s\S]*?)-->/)[1]
    .replace(
        '{{OG_JPEG_BASE64}}',
        readFileSync('../../../components/secutils-docs/static/img/og/og-mars.jpg').toString('base64'),
    );
// The in-memory key-value store: values, and expiry times in seconds since the epoch.
const values = new Map(),
    expiry = new Map();
// Decodes a KV value. The store accepts canonical base64url only and throws for other text.
const binary = (value) => {
    const bytes = Buffer.from(value, 'base64url');
    if (bytes.toString('base64url') !== value) throw new Error('Invalid base64url value');
    return bytes;
};
// True when the key exists and has not expired.
const live = (k) => values.has(k) && (expiry.get(k) ?? Infinity) > Date.now() / 1000;
// The `secutils.kv` API of the responder runtime. A value holds at most 1 MiB of bytes.
const kv = {
    get: async (k) => (live(k) ? values.get(k) : null),
    getEntry: async (k) => ({
        value: live(k) ? values.get(k) : null,
        expiresAt: live(k) ? (expiry.get(k) ?? null) : null,
    }),
    set: async (k, v, o = {}) => {
        if (binary(v).length > 1048576) throw new Error('KV value cap');
        if (o.ifAbsent && live(k)) return;
        values.set(k, v);
        expiry.set(k, o.ttlSec ? Math.floor(Date.now() / 1000) + o.ttlSec : undefined);
    },
    delete: async (k) => values.delete(k),
    list: async ({ prefix = '', after = '', limit = 200, valuesIncluded = true } = {}) => {
        const entries = [...values.keys()]
            .filter((k) => k.startsWith(prefix) && k > after && live(k))
            .sort()
            .slice(0, Math.min(limit, 1000))
            .map((key) => ({ key, ...(valuesIncluded ? { value: values.get(key) } : {}) }));
        return { entries, cursor: entries.at(-1)?.key ?? null };
    },
};
// `sha256` takes base64url bytes and returns the hex digest.
const secutils = { kv, crypto: { sha256: (b64) => createHash('sha256').update(binary(b64)).digest('hex') } };

createServer(async (req, res) => {
    const url = new URL(req.url, `http://localhost:${PORT}`);
    if (url.pathname === '/llms.txt') {
        res.writeHead(200, { 'Content-Type': 'text/markdown; charset=utf-8' });
        res.end(readFileSync('SKILL.md', 'utf8').replace(/\{\{\s*MARS_HOST\s*\}\}/g, `localhost:${PORT}`));
        return;
    }
    const chunks = [];
    try {
        for await (const c of req) chunks.push(c);
    } catch (error) {
        if (req.aborted || error.code === 'ECONNRESET') return;
        res.writeHead(400);
        res.end();
        return;
    }
    // Each request gets a budget of 200 KV operations, like the responder runtime.
    let operations = 0;
    const bounded = new Proxy(kv, {
        get(target, property) {
            const value = target[property];
            return (...args) => {
                if (++operations > 200) throw new Error('KV operation budget');
                return value(...args);
            };
        },
    });
    // In test mode (`MARS_SYNC_TEST_MODE=1`) the `x-mars-test-client` header sets the client address.
    const testAddress = process.env.MARS_SYNC_TEST_MODE === '1' ? req.headers['x-mars-test-client'] : undefined;
    // The globals that a responder script sees: `context`, `secutils`, and `Deno.core`.
    const sandbox = {
        context: {
            method: req.method,
            path: url.pathname,
            query: Object.fromEntries(url.searchParams),
            headers: req.headers,
            body: new Uint8Array(Buffer.concat(chunks)),
            clientAddress: testAddress || '127.0.0.1:0',
            params: { secrets: { MARS_STATS_API_KEY: process.env.MARS_STATS_API_KEY || 'local-operator-secret' } },
        },
        secutils: { ...secutils, kv: bounded },
        Uint8Array,
        Deno: { core: { encode: (s) => new TextEncoder().encode(s), decode: (b) => new TextDecoder().decode(b) } },
        console,
    };
    sandbox.globalThis = sandbox;
    vm.createContext(sandbox);
    // Sync paths and test-scope requests run the assembled script. Other requests run the
    // snapshot script alone.
    const script =
        url.pathname.startsWith('/sync/') || req.headers['x-mars-test-scope'] || req.headers['x-mars-test-token']
            ? inlineMarsSync(legacyScript, readFileSync('dist/mars-sync-responder.js', 'utf8'))
            : legacyScript;
    let out;
    try {
        vm.runInContext(`globalThis.__ret = ${script}`, sandbox);
        out = await sandbox.__ret;
    } catch (error) {
        console.error('Preview responder failed:', error.message);
        res.writeHead(500, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: 'preview-responder-failed' }));
        return;
    }
    // A script result is the response. Null serves the page.
    if (out) {
        res.writeHead(out.statusCode, out.headers);
        res.end(out.body);
        return;
    }
    // Same cross-origin isolation headers as dev/tools/shared/deploy.ts.
    res.writeHead(200, {
        'Content-Type': 'text/html; charset=utf-8',
        'Cross-Origin-Opener-Policy': 'same-origin',
        'Cross-Origin-Embedder-Policy': 'credentialless',
    });
    res.end(readFileSync('dist/mars.html'));
}).listen(PORT, () => console.log(`MARS dev server: http://localhost:${PORT}/`));
