// Sharing: full links and encrypted short links for documents and folders, Slack/GitHub
// copies, and opening shared links into the recipient's tree ("Shared with me").
import * as app from '../app.ts';
import * as db from '../store/db.ts';
import { ancestors, descendants, textBytes, type TreeNode } from '../tree/model.ts';
import { $, copyRich, esc, modal } from '../ui/ui.ts';
import * as viewer from '../doc/viewer.js';
import { type Bundle, decodeState, deriveShortLink, encodeState, fromBase64Url, hashText, openSealed, parseBundle, sealBundle, SHORT_MAX_CHARS, toBase64Url } from './codec.ts';
import { downloadDoc } from '../io/io.ts';
import { mergeDocuments, splitComments, type Thread } from './merge.ts';

export const SHARED_FOLDER = 'Shared with me';
const SHORT_TTLS = [86400, 604800, 1209600];
const LINKS_KEY = 'mars.shortLinks';
const TTL_KEY = 'mars.shortTtl';
let shortTtl = 604800;
try { shortTtl = SHORT_TTLS.find((t) => t === Number(localStorage.getItem(TTL_KEY))) ?? shortTtl; } catch {}

const base = () => `${location.origin}${location.pathname}`;
const ttlLabel = (sec: number) => `${sec / 86400} day${sec === 86400 ? '' : 's'}`;
const fmtExpiry = (exp: number) => new Date(exp * 1000).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
const fmtSize = (chars: number) => (chars < 1024 ? `${chars} B` : chars < 1048576 ? `${Math.round(chars / 1024)} KB` : `${(chars / 1048576).toFixed(1)} MB`);

// ---- Bundles ----
async function bundleFor(id: string): Promise<Bundle | null> {
    const n = app.node(id);
    if (!n) return null;
    if (n.kind === 'doc') return { v: 1, kind: 'doc', name: n.name, docs: [{ path: [], name: n.name, originId: n.originId ?? n.id, text: await app.docText(n.id) }] };
    const docs = [];
    for (const d of descendants(app.state.nodes, n.id)) {
        if (d.kind !== 'doc') continue;
        const path = ancestors(app.state.nodes, d.id).slice(ancestors(app.state.nodes, n.id).length + 1).map((f) => f.name);
        docs.push({ path, name: d.name, originId: d.originId ?? d.id, text: await app.docText(d.id) });
    }
    return { v: 1, kind: 'folder', name: n.name, originId: n.originId ?? n.id, docs };
}

async function fullLink(bundle: Bundle) {
    if (bundle.kind === 'doc') {
        const d = bundle.docs[0];
        return `${base()}#${new URLSearchParams({ name: d.name, md: await encodeState(d.text), o: d.originId })}`;
    }
    return `${base()}#b=${await encodeState(JSON.stringify(bundle))}`;
}

// ---- Short links (stored by the responder script in mars.html) ----
type LinkRecord = { token: string; exp: number; secret: string; ttl: number; hash: string; nodeId: string };

function readLinks(): Record<string, LinkRecord> {
    let links: Record<string, LinkRecord> = {};
    try { links = JSON.parse(localStorage.getItem(LINKS_KEY) ?? '{}') || {}; } catch {}
    const now = Date.now() / 1000;
    for (const [id, l] of Object.entries(links)) if (!(l?.exp > now)) delete links[id];
    return links;
}
const writeLinks = (links: Record<string, LinkRecord>) => { try { localStorage.setItem(LINKS_KEY, JSON.stringify(links)); } catch {} };

async function shortFetch(id: string, { query = '', ...init }: RequestInit & { query?: string } = {}) {
    const send = () => fetch(`${location.pathname}?s=${id}${query}`, { cache: 'no-store', ...init }).catch(() => {
        throw new Error('Network error. Check your connection and try again.');
    });
    const res = await send();
    // A 429 without a JSON error comes from the responder's concurrency limit, not the throttle.
    if (res.status !== 429 || (await res.clone().json().catch(() => null))?.error) return res;
    await new Promise((r) => setTimeout(r, 1000 * Math.min(5, Number(res.headers.get('Retry-After')) || 1)));
    return send();
}

async function shortLink(nodeId: string, bundle: Bundle) {
    const hash = await hashText(JSON.stringify(bundle)), ttl = shortTtl, now = Date.now() / 1000;
    const links = readLinks();
    const reuse = Object.values(links).find((l) => l.hash === hash && l.ttl === ttl && l.exp - now >= ttl / 2);
    if (reuse) return { url: `${base()}#s=${reuse.secret}`, exp: reuse.exp };
    const { id, secret, blob } = await sealBundle(bundle);
    if (blob.length > SHORT_MAX_CHARS) throw new Error('Too large for a short link. Copy a full link or download the file instead.');
    const token = toBase64Url(crypto.getRandomValues(new Uint8Array(16)));
    const res = await shortFetch(id, { method: 'POST', body: blob, headers: { 'x-su-delete-token': token }, query: `&ttl=${ttl}` });
    if (res.status !== 201) {
        throw new Error(({
            413: 'Too large for a short link. Copy a full link or download the file instead.',
            429: 'Too many short links from this network. Try again in an hour.',
            507: 'Short-link storage is full right now. Copy a full link instead.',
        } as Record<number, string>)[res.status] ?? `Could not create a short link (HTTP ${res.status})`);
    }
    const { exp } = await res.json();
    links[id] = { token, exp, secret, ttl, hash, nodeId };
    writeLinks(links);
    return { url: `${base()}#s=${secret}`, exp };
}

// Copies a link for a tree node. The clipboard write starts synchronously (Safari).
export async function copyNodeLink(id: string, kind: 'short' | 'full') {
    if (id === app.state.currentId) await viewer.flush();
    const bundle = bundleFor(id);
    const url = bundle.then(async (b) => {
        if (!b) throw new Error('Nothing to share');
        return kind === 'full' ? fullLink(b) : (await shortLink(id, b)).url;
    });
    if (kind === 'short') viewer.toast('Creating short link...');
    try {
        await copyRich(url);
        viewer.toast(kind === 'full' ? 'Full link copied' : `Short link copied · expires in ${ttlLabel(shortTtl)}`);
    } catch (err) {
        viewer.toast((await url.then(() => null, (e) => e?.message)) ?? 'Failed to copy the link');
    }
}

// Short link when possible, full link otherwise (too large, offline, or throttled).
async function bestLink(id: string, bundle: Bundle) {
    try { return (await shortLink(id, bundle)).url; } catch { return fullLink(bundle); }
}

function githubSummary(name: string, url: string, threads: Thread[]) {
    const open = threads.filter((t) => !t.resolved);
    const quote = (s: string) => s.split('\n').map((l) => `> ${l}`).join('\n');
    const lines = [`**[${name.replace(/[[\]]/g, '\\$&')}](${url})** · review in MARS · ${open.length} open thread${open.length === 1 ? '' : 's'}`];
    for (const t of open) {
        lines.push('', '---', '', quote(t.quote.length > 400 ? `${t.quote.slice(0, 400)}…` : t.quote));
        for (const c of t.comments) lines.push('', `**${c.author || 'Anonymous'}**${c.at ? ` · ${c.at.slice(0, 10)}` : ''}`, '', c.body);
    }
    return lines.join('\n');
}

// ---- Toolbar share menu (current document) ----
const menu = $('shareMenu');
const hideMenu = () => { if (menu.matches(':popover-open')) menu.hidePopover(); };

async function currentBundle() {
    await viewer.flush();
    const id = app.state.currentId;
    const bundle = id ? await bundleFor(id) : null;
    if (!id || !bundle) throw new Error('Nothing to share yet');
    return { id, bundle };
}

function renderMenu(tooLarge = false) {
    for (const b of $('shortTtl').querySelectorAll<HTMLButtonElement>('button')) b.setAttribute('aria-pressed', String(Number(b.dataset.ttl) === shortTtl));
    ($('copyShortLinkBtn') as HTMLButtonElement).disabled = tooLarge;
    $('shortTtl').hidden = tooLarge;
    $('shortLinkSub').textContent = tooLarge ? 'Too large for a short link; copy a full link or download' : `Expires in ${ttlLabel(shortTtl)} · encrypted copy stored on Secutils.dev`;
    const owned = Object.values(readLinks()).filter((l) => l.nodeId === app.state.currentId);
    $('deleteShortLinkBtn').hidden = !owned.length;
    if (owned.length) $('deleteShortLinkLabel').textContent = `Delete short link${owned.length > 1 ? 's' : ''} · expires ${fmtExpiry(Math.max(...owned.map((l) => l.exp)))}`;
}

menu.addEventListener('toggle', async (e) => {
    if ((e as ToggleEvent).newState !== 'open') return;
    renderMenu();
    try {
        const { bundle } = await currentBundle();
        const full = await fullLink(bundle);
        $('fullLinkSub').textContent = `Never expires · whole document in the URL (${fmtSize(full.length)})`;
        renderMenu(full.length > SHORT_MAX_CHARS * 1.05);
    } catch {}
});
$('shortTtl').addEventListener('click', (e) => {
    const ttl = Number((e.target as HTMLElement).closest<HTMLElement>('button[data-ttl]')?.dataset.ttl);
    if (!SHORT_TTLS.includes(ttl)) return;
    shortTtl = ttl;
    try { localStorage.setItem(TTL_KEY, String(ttl)); } catch {}
    renderMenu();
});
$('copyShortLinkBtn').addEventListener('click', () => { hideMenu(); if (app.state.currentId) copyNodeLink(app.state.currentId, 'short'); });
$('copyFullLinkBtn').addEventListener('click', () => { hideMenu(); if (app.state.currentId) copyNodeLink(app.state.currentId, 'full'); });
$('downloadMdBtn').addEventListener('click', async () => {
    hideMenu();
    await viewer.flush();
    if (app.state.currentId) downloadDoc(app.state.currentId);
});
$('copySlackBtn').addEventListener('click', async () => {
    hideMenu();
    const data = currentBundle().then(async ({ id, bundle }) => {
        const url = await bestLink(id, bundle);
        const open = splitComments(bundle.docs[0].text).threads.filter((t) => !t.resolved).length;
        const label = `${bundle.name} (MARS review${open ? `, ${open} open thread${open === 1 ? '' : 's'}` : ''})`;
        return { plain: `${label}: ${url}`, html: `<a href="${esc(url)}">${esc(label)}</a>` };
    });
    try {
        await copyRich(data.then((d) => d.plain), data.then((d) => d.html));
        viewer.toast('Copied for Slack: paste it into a message');
    } catch (err) { viewer.toast((err as Error)?.message ?? 'Failed to copy'); }
});
$('copyGithubBtn').addEventListener('click', async () => {
    hideMenu();
    const text = currentBundle().then(async ({ id, bundle }) => githubSummary(bundle.name, await bestLink(id, bundle), splitComments(bundle.docs[0].text).threads));
    try {
        await copyRich(text);
        viewer.toast('Copied a GitHub comment with the open threads');
    } catch (err) { viewer.toast((err as Error)?.message ?? 'Failed to copy'); }
});
$('deleteShortLinkBtn').addEventListener('click', async () => {
    hideMenu();
    const links = readLinks();
    try {
        for (const [id, l] of Object.entries(links)) {
            if (l.nodeId !== app.state.currentId) continue;
            const res = await shortFetch(id, { method: 'DELETE', headers: { 'x-su-delete-token': l.token } });
            if (res.status !== 204 && res.status !== 404) throw new Error(`Could not delete the short link (HTTP ${res.status})`);
            delete links[id];
        }
        viewer.toast('Short link deleted');
    } catch (err) { viewer.toast((err as Error).message); }
    finally { writeLinks(links); }
});

// ---- Receiving ----
const findDoc = (originId: string) => app.state.nodes.find((n) => n.kind === 'doc' && (n.originId ?? n.id) === originId);
const sharedRoot = () => app.findOrCreateFolder(SHARED_FOLDER, null);

async function saveText(n: TreeNode, text: string) {
    n.size = textBytes(text);
    await db.putNodes([n], { [n.id]: text });
    if (n.id === app.state.currentId) viewer.openDocument({ text, name: n.name, handle: n.handle ?? null, stamp: n.stamp ?? 0, dirty: !!n.dirty });
}

async function receiveDoc(doc: Bundle['docs'][0]) {
    const existing = findDoc(doc.originId);
    if (!existing) {
        const parent = await sharedRoot();
        await app.createDoc({ name: app.uniqueName(doc.name || 'Untitled', parent.id), text: doc.text, parentId: parent.id, originId: doc.originId });
        viewer.toast(`Added "${doc.name}" to ${SHARED_FOLDER}`);
        return;
    }
    const local = (await db.getText(existing.id)) ?? '';
    const merged = mergeDocuments(local, doc.text);
    if (merged.sameBody || local === doc.text) {
        if (merged.added) await saveText(existing, merged.mine);
        await app.openDoc(existing.id);
        viewer.toast(merged.added ? `Merged ${merged.added} new comment${merged.added === 1 ? '' : 's'} into "${existing.name}"` : `"${existing.name}" is already up to date`);
        return;
    }
    await app.openDoc(existing.id);
    const { value } = await modal({
        title: 'This document is already in your sidebar',
        html: `<p>The shared copy of <strong>${esc(existing.name)}</strong> has different text than yours${merged.added ? ` and ${merged.added} new comment${merged.added === 1 ? '' : 's'}` : ''}. Comments are merged in every case.</p>`,
        buttons: [{ label: 'Save as a copy', value: 'copy' }, { label: 'Use shared text', value: 'theirs' }, { label: 'Keep my text', value: 'mine', primary: true }],
    });
    if (value === 'copy') {
        const parent = await sharedRoot();
        await app.createDoc({ name: app.uniqueName(`${doc.name} (shared)`, parent.id), text: doc.text, parentId: parent.id });
    } else if (value) {
        await saveText(existing, value === 'theirs' ? merged.theirs : merged.mine);
        viewer.toast(value === 'theirs' ? 'Replaced the text with the shared version' : 'Kept your text and merged the comments');
    }
}

async function receiveFolder(bundle: Bundle) {
    let target = bundle.originId ? app.state.nodes.find((n) => n.kind === 'folder' && n.originId === bundle.originId) : undefined;
    if (!target) {
        const parent = await sharedRoot();
        target = await app.createFolder(app.uniqueName(bundle.name || 'Untitled', parent.id), parent.id, bundle.originId);
    }
    let added = 0, merged = 0, differ = 0, first: string | null = null;
    for (const doc of bundle.docs) {
        const existing = findDoc(doc.originId);
        if (existing) {
            const local = (await db.getText(existing.id)) ?? '';
            const m = mergeDocuments(local, doc.text);
            if (!m.sameBody) differ++;
            if (m.added) { merged += m.added; await saveText(existing, m.mine); }
            first ??= existing.id;
            continue;
        }
        let parentId = target.id;
        for (const seg of doc.path) parentId = (await app.findOrCreateFolder(seg, parentId)).id;
        const created = await app.createDoc({ name: app.uniqueName(doc.name, parentId), text: doc.text, parentId, originId: doc.originId, open: false });
        first ??= created.id;
        added++;
    }
    await app.setOpen(target.id, true);
    if (first) await app.openDoc(first);
    const parts = [added && `added ${added} document${added === 1 ? '' : 's'}`, merged && `merged ${merged} comment${merged === 1 ? '' : 's'}`, differ && `kept your text in ${differ} changed document${differ === 1 ? '' : 's'}`].filter(Boolean);
    viewer.toast(`"${target.name}": ${parts.join(', ') || 'already up to date'}`);
}

export async function receive(bundle: Bundle) {
    if (bundle.kind === 'doc' && bundle.docs.length === 1) await receiveDoc(bundle.docs[0]);
    else await receiveFolder(bundle);
}

// Fetches a remote Markdown file (?url=, or the empty-state URL box) into the tree.
export async function receiveUrl(url: string) {
    const res = await fetch(url, { redirect: 'follow' });
    if (!res.ok) throw new Error(`Fetch failed: HTTP ${res.status}`);
    const name = decodeURIComponent(new URL(url).pathname.split('/').pop() || 'document').replace(/\.(md|markdown|mdx|txt)$/i, '') || 'document';
    await receive({ v: 1, kind: 'doc', name, docs: [{ path: [], name, originId: `url:${url}`, text: await res.text() }] });
}

// Opens whatever the address bar carries: `#doc=<id>`, a share link, or `?url=`.
// Returns false when there was nothing to open.
export async function openFromLocation(): Promise<boolean> {
    const hash = location.hash.slice(1);
    const params = new URLSearchParams(hash.includes('=') ? hash : '');
    const urlParam = new URLSearchParams(location.search).get('url');
    const fail = (msg: string | null) => {
        history.replaceState(history.state, '', location.pathname);
        viewer.endLoading(!msg);
        if (msg) viewer.toast(msg);
        return true;
    };
    try {
        if (params.has('doc')) {
            const id = params.get('doc')!;
            if (app.node(id)?.kind === 'doc') { await app.openDoc(id); return true; }
            history.replaceState(history.state, '', location.pathname);
            return false;
        }
        let bundle: Bundle | null = null;
        if (params.has('s')) {
            const secret = params.get('s')!;
            let id: string;
            try { id = (await deriveShortLink(fromBase64Url(secret))).id; } catch { return fail(null); }
            const res = await shortFetch(id);
            if (res.status === 404) return fail(null);
            if (!res.ok) return fail(`Could not load the short link (HTTP ${res.status}). Reload to retry.`);
            const { v, exp } = await res.json();
            bundle = await openSealed(secret, v);
            if (!bundle) return fail(null);
            if (exp) viewer.toast(`Shared link · expires ${fmtExpiry(exp)}`);
        } else if (params.has('b')) {
            const json = await decodeState(params.get('b')!);
            bundle = json === null ? null : parseBundle(json);
            if (!bundle) return fail('This link is damaged or incomplete.');
        } else if (hash) {
            // md.html-compatible full links: `#<state>` or `#name=<name>&md=<state>[&o=<originId>]`.
            const text = await decodeState(params.get('md') ?? (hash.includes('=') ? '' : hash));
            if (text === null) return fail('This link is damaged or incomplete.');
            const name = params.get('name')?.trim().replace(/\.(md|markdown|mdx|txt)$/i, '') || text.match(/^#\s+(.+)$/m)?.[1]?.trim().slice(0, 80) || 'Untitled';
            bundle = { v: 1, kind: 'doc', name, docs: [{ path: [], name, originId: params.get('o') || `link:${await hashText(text)}`, text }] };
        } else if (urlParam) {
            try { await receiveUrl(urlParam); } catch (err) { return fail(`${(err as Error).message}. The URL must be public and CORS-enabled.`); }
            return true;
        } else {
            return false;
        }
        await receive(bundle);
        return true;
    } catch (err) {
        return fail(`Could not open the link: ${(err as Error)?.message ?? err}`);
    }
}
