// Local dev server: serves dist/mars.html (from `npm run preview`) at / and the skill at
// /llms.txt, like the MARS host, and runs the page's `@su:responder-script` against an
// in-memory KV, so short links work offline.
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import * as vm from 'node:vm';

const PORT = Number(process.env.PORT) || 7373;
const script = readFileSync('../../mars.html', 'utf8').match(/<!--\s*@su:responder-script\s*([\s\S]*?)-->/)[1]
    .replace('{{OG_JPEG_BASE64}}', readFileSync('../../../../components/secutils-docs/static/img/og/og-mars.jpg').toString('base64'));
const values = new Map(), expiry = new Map();
const live = (k) => values.has(k) && (expiry.get(k) ?? Infinity) > Date.now() / 1000;
const kv = {
    get: async (k) => (live(k) ? values.get(k) : null),
    getEntry: async (k) => ({ value: live(k) ? values.get(k) : null, expiresAt: live(k) ? expiry.get(k) ?? null : null }),
    set: async (k, v, o = {}) => {
        if (o.ifAbsent && live(k)) return;
        values.set(k, v);
        expiry.set(k, o.ttlSec ? Math.floor(Date.now() / 1000) + o.ttlSec : undefined);
    },
    delete: async (k) => values.delete(k),
    list: async ({ prefix = '', limit = 200 } = {}) => ({ entries: [...values.keys()].filter((k) => k.startsWith(prefix) && live(k)).slice(0, limit).map((key) => ({ key })), cursor: null }),
};
const secutils = { kv, crypto: { sha256: (b64) => createHash('sha256').update(Buffer.from(b64, 'base64url')).digest('hex') } };

createServer(async (req, res) => {
    const url = new URL(req.url, `http://localhost:${PORT}`);
    if (url.pathname === '/llms.txt') { res.writeHead(200, { 'Content-Type': 'text/markdown; charset=utf-8' }); res.end(readFileSync('../../mars.skill.md', 'utf8').replace(/\{\{\s*MARS_HOST\s*\}\}/g, `localhost:${PORT}`)); return; }
    const chunks = [];
    for await (const c of req) chunks.push(c);
    const sandbox = { context: { method: req.method, path: url.pathname, query: Object.fromEntries(url.searchParams), headers: req.headers, body: new Uint8Array(Buffer.concat(chunks)), clientAddress: '127.0.0.1:0' }, secutils, Deno: { core: { decode: (b) => new TextDecoder().decode(b) } }, console };
    sandbox.globalThis = sandbox;
    vm.createContext(sandbox);
    vm.runInContext(`globalThis.__ret = ${script}`, sandbox);
    const out = await sandbox.__ret;
    if (out) { res.writeHead(out.statusCode, out.headers); res.end(out.body); return; }
    // Same cross-origin isolation headers as dev/tools/deploy.ts.
    res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8', 'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'credentialless' });
    res.end(readFileSync('dist/mars.html'));
}).listen(PORT, () => console.log(`MARS dev server: http://localhost:${PORT}/`));
