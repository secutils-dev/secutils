// Sharing: full links and encrypted short links for documents and folders, Slack/GitHub
// copies, and opening shared links into the recipient's tree ("Shared with me").
//
// Owns snapshot links, the registry of short links that this browser created, the Share menu,
// and the receive path for links, files, and URLs. Does not own the wire formats (`codec.ts`),
// the Copy link dialog (`dialog.ts`), or Live links (`sync/`).
// Trust:
// - A fragment, a fetched URL, a file, and a service response are untrusted. Links decode through
//   `codec.ts`, which bounds the size and checks the shape. Document names pass through `esc()`
//   before they enter dialog markup.
// - The short-link service receives the derived id, the ciphertext, a delete token, and the
//   lifetime. The `#s=` secret stays in the fragment.
// Each receive step takes the `InputOperation` of its request and checks it after every wait.
import * as app from '../app.ts';
import * as db from '../store/db.ts';
import { ancestors, descendants, textBytes, type TreeNode } from '../tree/model.ts';
import { $, copyRich, esc, modal } from '../ui/ui.ts';
import * as viewer from '../doc/viewer.js';
import {
    type Bundle,
    decodeState,
    deriveShortLink,
    encodeState,
    fromBase64Url,
    hashText,
    openSealed,
    parseBundle,
    sealBundle,
    SHORT_MAX_CHARS,
    toBase64Url,
} from './codec.ts';
import { joinComments, mergeDocuments, openThreads, splitComments, type Thread } from './merge.ts';
import { matchScore } from './match.ts';
import { beginInput, checkInput, type InputOperation } from '../io/input.ts';
import { readSource, sourceUrl, urlFileName } from '../io/source.ts';
import { validWidth, type ReadingWidth } from '../export/settings.ts';
import { temporaryFragment } from './temporary.ts';
import * as sync from '../sync/index.ts';

/** Name of the top-level folder that receives shared documents when no default folder is set. */
export const SHARED_FOLDER = 'Shared with me';
// Short-link lifetimes in seconds: 1, 7, 14, and 30 days. The responder script accepts the same values.
const SHORT_TTLS = [86400, 604800, 1209600, 2592000];
// `localStorage` key of the short-link registry: service id -> `LinkRecord`.
const LINKS_KEY = 'mars.shortLinks';
// `localStorage` key of the lifetime that the reader chose last, in seconds.
const TTL_KEY = 'mars.shortTtl';
// The default lifetime is 7 days.
let shortTtl = 604800;
try {
    shortTtl = SHORT_TTLS.find((t) => t === Number(localStorage.getItem(TTL_KEY))) ?? shortTtl;
} catch {}

// The page address without query and fragment. Every generated link starts with it.
const base = () => `${location.origin}${location.pathname}`;
const ttlLabel = (sec: number) => `${sec / 86400} day${sec === 86400 ? '' : 's'}`;
// `exp` is in seconds since the epoch.
const fmtExpiry = (exp: number) =>
    new Date(exp * 1000).toLocaleString(undefined, {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit',
    });
const fmtSize = (chars: number) =>
    chars < 1024
        ? `${chars} B`
        : chars < 1048576
          ? `${Math.round(chars / 1024)} KB`
          : `${(chars / 1048576).toFixed(1)} MB`;

// ---- Bundles ----
/**
 * Builds the bundle of a document, or of a folder with every document below it. Returns null
 * for an unknown id. The open document contributes its viewer text and width. Callers flush the
 * viewer first, so that text includes a pending edit. A temporary session marks the bundle `temporary`.
 */
async function bundleFor(id: string): Promise<Bundle | null> {
    const n = app.node(id);
    if (!n) return null;
    const widthFor = (doc: TreeNode) =>
        doc.id === app.state.currentId
            ? viewer.exportReadingWidth()
            : (doc.readingWidth ?? viewer.defaultReadingWidth());
    if (n.kind === 'doc')
        return {
            v: 1,
            kind: 'doc',
            name: n.name,
            docs: [
                {
                    path: [],
                    name: n.name,
                    originId: n.originId ?? n.id,
                    text: await app.docText(n.id),
                    readingWidth: widthFor(n),
                },
            ],
            ...(db.isTemporaryStorage() ? { temporary: true } : {}),
        };
    const docs = [];
    for (const d of descendants(app.state.nodes, n.id)) {
        if (d.kind !== 'doc') continue;
        // Folder names between the shared folder and the document.
        const path = ancestors(app.state.nodes, d.id)
            .slice(ancestors(app.state.nodes, n.id).length + 1)
            .map((f) => f.name);
        const readingWidth = widthFor(d);
        docs.push({ path, name: d.name, originId: d.originId ?? d.id, text: await app.docText(d.id), readingWidth });
    }
    return {
        v: 1,
        kind: 'folder',
        name: n.name,
        originId: n.originId ?? n.id,
        docs,
        ...(db.isTemporaryStorage() ? { temporary: true } : {}),
    };
}

/**
 * Builds a full link: the content travels in the fragment, and nothing is uploaded.
 * A document uses the Markdown Preview form (`name`, `md`, `o`, optional `w`). A folder uses `#b=`.
 */
async function fullLink(bundle: Bundle) {
    if (bundle.kind === 'doc') {
        const d = bundle.docs[0];
        const params = new URLSearchParams({ name: d.name, md: await encodeState(d.text), o: d.originId });
        if (validWidth(d.readingWidth)) params.set('w', String(d.readingWidth));
        if (bundle.temporary) params.set('t', '1');
        return `${base()}#${params}`;
    }
    return `${base()}#b=${await encodeState(JSON.stringify(bundle))}${bundle.temporary ? '&t=1' : ''}`;
}

// ---- Short links (stored by the responder script in index.html) ----
/**
 * One short link that this browser created.
 * `token` deletes the link early. The service keeps only its hash. `secret` rebuilds the URL
 * when the link is reused. `hash` identifies the shared content. `exp` is in seconds since
 * the epoch, and `ttl` in seconds.
 */
type LinkRecord = { token: string; exp: number; secret: string; ttl: number; hash: string; nodeId: string };
// The registry of a temporary session. Its delete tokens stay in memory.
let temporaryLinks: Record<string, LinkRecord> = {};

// Returns the unexpired records of the active registry. Unreadable storage gives an empty registry.
function readLinks(): Record<string, LinkRecord> {
    let links: Record<string, LinkRecord> = {};
    if (db.isTemporaryStorage()) links = { ...temporaryLinks };
    else
        try {
            links = JSON.parse(localStorage.getItem(LINKS_KEY) ?? '{}') || {};
        } catch {}
    const now = Date.now() / 1000;
    for (const [id, l] of Object.entries(links)) if (!(l?.exp > now)) delete links[id];
    return links;
}
const writeLinks = (links: Record<string, LinkRecord>) => {
    if (db.isTemporaryStorage()) {
        temporaryLinks = links;
        return;
    }
    try {
        localStorage.setItem(LINKS_KEY, JSON.stringify(links));
    } catch {}
};
/**
 * Folder snapshots can also contain a selected descendant. Count each known,
 * unexpired snapshot once; deleting a library item does not revoke snapshots.
 * Returns the number of short links of this browser that include one of `ids`.
 */
export function shortLinksForDeletion(ids: string[]) {
    const selected = new Set(ids.flatMap((id) => [id, ...ancestors(app.state.nodes, id).map((node) => node.id)]));
    return Object.values(readLinks()).filter((link) => selected.has(link.nodeId)).length;
}
const shortUrl = (secret: string, bundle: Bundle) => `${base()}#s=${secret}${bundle.temporary ? '&t=1' : ''}`;

/**
 * Sends one request to the short-link endpoint of this page (`?s=<id>`). The response is never cached.
 * A network failure throws an `Error` with a display message. An HTTP error status returns normally.
 * After a 429 from the concurrency limit, it waits and sends the request once more.
 * The wait is `Retry-After`, at most 5 seconds, and 1 second by default.
 */
async function shortFetch(id: string, { query = '', ...init }: RequestInit & { query?: string } = {}) {
    const send = () =>
        fetch(`${location.pathname}?s=${id}${query}`, { cache: 'no-store', ...init }).catch(() => {
            throw new Error('Network error. Check your connection and try again.');
        });
    const res = await send();
    // A 429 without a JSON error comes from the responder's concurrency limit, not the throttle.
    if (
        res.status !== 429 ||
        (
            await res
                .clone()
                .json()
                .catch(() => null)
        )?.error
    )
        return res;
    await new Promise((r) => setTimeout(r, 1000 * Math.min(5, Number(res.headers.get('Retry-After')) || 1)));
    return send();
}

/**
 * Returns a short link for `bundle`, with `exp` in seconds since the epoch.
 * It reuses a link of this browser with the same content and lifetime while at least half of
 * that lifetime remains. Otherwise it encrypts the bundle, uploads the blob, and records the
 * link with its delete token. Throws an `Error` with a display message when the blob exceeds
 * `SHORT_MAX_CHARS` and when the service refuses the upload.
 */
async function shortLink(nodeId: string, bundle: Bundle, ttl = shortTtl) {
    const hash = await hashText(JSON.stringify(bundle)),
        now = Date.now() / 1000;
    const links = readLinks();
    const reuse = Object.values(links).find((l) => l.hash === hash && l.ttl === ttl && l.exp - now >= ttl / 2);
    if (reuse) return { url: shortUrl(reuse.secret, bundle), exp: reuse.exp };
    const { id, secret, blob } = await sealBundle(bundle);
    if (blob.length > SHORT_MAX_CHARS)
        throw new Error('Too large for a short link. Copy a full link or download the file instead.');
    // A random 128-bit delete token. Only this browser holds it, so only it can delete the link early.
    const token = toBase64Url(crypto.getRandomValues(new Uint8Array(16)));
    const res = await shortFetch(id, {
        method: 'POST',
        body: blob,
        headers: { 'x-su-delete-token': token },
        query: `&ttl=${ttl}`,
    });
    if (res.status !== 201) {
        throw new Error(
            (
                {
                    413: 'Too large for a short link. Copy a full link or download the file instead.',
                    429: 'Too many short links from this network. Try again in an hour.',
                    507: 'Short-link storage is full right now. Copy a full link instead.',
                } as Record<number, string>
            )[res.status] ?? `Could not create a short link (HTTP ${res.status})`,
        );
    }
    const { exp } = await res.json();
    links[id] = { token, exp, secret, ttl, hash, nodeId };
    writeLinks(links);
    return { url: shortUrl(secret, bundle), exp };
}
/**
 * Creates a snapshot link for a node. Pending edits of the open document are saved first.
 * `full` uploads nothing and returns `expires: null`. `short` requires a lifetime from
 * `SHORT_TTLS`, stores it as the preference, and returns `expires` in seconds since the epoch.
 * Throws an `Error` with a display message.
 */
export async function snapshotLink(id: string, format: 'short' | 'full', ttl = shortTtl) {
    if (id === app.state.currentId || descendants(app.state.nodes, id).some((node) => node.id === app.state.currentId))
        await viewer.flush();
    const bundle = await bundleFor(id);
    if (!bundle) throw new Error('Nothing to share');
    if (format === 'full') return { url: await fullLink(bundle), expires: null };
    if (!SHORT_TTLS.includes(ttl)) throw new Error('Choose an available expiration date.');
    shortTtl = ttl;
    try {
        localStorage.setItem(TTL_KEY, String(ttl));
    } catch {}
    const result = await shortLink(id, bundle, ttl);
    return { url: result.url, expires: result.exp };
}
/** Opens the Copy link dialog. The dialog module loads on first use. */
export const copyLinkDialog = (id: string, mode: 'snapshot' | 'live' = 'snapshot') =>
    import('./dialog.ts').then((module) => module.openShareDialog(id, mode));

/**
 * Copies a link for a tree node. The clipboard write starts synchronously (Safari).
 * A short link uses the stored lifetime preference. The result and a failure show as a toast.
 */
export async function copyNodeLink(id: string, kind: 'short' | 'full') {
    let sharedId = id;
    const bundle = (async () => {
        if (
            id === app.state.currentId ||
            descendants(app.state.nodes, id).some((node) => node.id === app.state.currentId)
        ) {
            await viewer.flush();
            if (id === app.state.currentId) sharedId = app.state.currentId ?? id;
        }
        return bundleFor(sharedId);
    })();
    const url = bundle.then(async (b) => {
        if (!b) throw new Error('Nothing to share');
        return kind === 'full' ? fullLink(b) : (await shortLink(sharedId, b)).url;
    });
    if (kind === 'short') viewer.toast('Creating short link...');
    try {
        await copyRich(url);
        viewer.toast(kind === 'full' ? 'Full link copied' : `Short link copied · expires in ${ttlLabel(shortTtl)}`);
    } catch (err) {
        viewer.toast(
            (await url.then(
                () => null,
                (e) => e?.message,
            )) ?? 'Failed to copy the link',
        );
    }
}

// Short link when possible, full link otherwise (too large, offline, or throttled).
async function bestLink(id: string, bundle: Bundle) {
    try {
        return (await shortLink(id, bundle)).url;
    } catch {
        return fullLink(bundle);
    }
}

// A GitHub comment in Markdown: a link line, then each open thread with its quote (at most
// 400 characters) and its comments.
function githubSummary(name: string, url: string, threads: Thread[]) {
    const open = threads.filter((t) => !t.resolved);
    const quote = (s: string) =>
        s
            .split('\n')
            .map((l) => `> ${l}`)
            .join('\n');
    const lines = [
        `**[${name.replace(/[[\]]/g, '\\$&')}](${url})** · review in MARS · ${open.length} open thread${open.length === 1 ? '' : 's'}`,
    ];
    for (const t of open) {
        lines.push('', '---', '', quote(t.quote.length > 400 ? `${t.quote.slice(0, 400)}…` : t.quote));
        for (const c of t.comments)
            lines.push('', `**${c.author || 'Anonymous'}**${c.at ? ` · ${c.at.slice(0, 10)}` : ''}`, '', c.body);
    }
    return lines.join('\n');
}

// A prompt that hands the open threads to an AI agent. The protocol itself lives in the
// skill (SKILL.md -> "Address review threads"); this is its paste-ready summary.
// An origin of the form `file:<path>` names the file that the agent must edit.
function agentPrompt(n: TreeNode, threads: Thread[]) {
    const open = threads.filter((t) => !t.resolved);
    const file = n.originId?.startsWith('file:') ? n.originId.slice(5) : null;
    const target = file ?? `the Markdown document "${n.name}" (ask me for its path if you don't know it)`;
    return [
        `Address the ${open.length} open review thread${open.length === 1 ? '' : 's'} on ${target}. The thread format is in the MARS skill: ${base()}llms.txt`,
        '',
        '1. Edit the document to resolve each thread, or decide that it needs no change.',
        '2. In each thread, append one comment with a new 8-hex-character "id", your name as "author", the current ISO 8601 time as "at", and a "body" that says what you changed or why you kept the text. Keep the thread "id".',
        '3. When you rewrite a quoted passage, set the thread\'s "quote", "prefix", and "suffix" to the new rendered text.',
        '4. Keep the su-comments block as the last thing in the file. If the file has no block, append this one with your replies:',
        '',
        joinComments('', open).trim(),
    ].join('\n');
}

// ---- Toolbar share menu (current document) ----
const menu = $('shareMenu');
const hideMenu = () => {
    if (menu.matches(':popover-open')) menu.hidePopover();
};

// The bundle of the open document, after a save of pending edits. Throws when no document is open.
async function currentBundle() {
    await viewer.flush();
    const id = app.state.currentId;
    const bundle = id ? await bundleFor(id) : null;
    if (!id || !bundle) throw new Error('Nothing to share yet');
    return { id, bundle };
}

/**
 * Starts a temporary session with a page load on `?temporary=1`, which selects the memory
 * backend before the database opens. `copyCurrent` carries the open document in the fragment
 * (Read temporarily). Otherwise `new=1` requests an empty document.
 */
export async function startTemporary(copyCurrent = false) {
    await viewer.flush();
    const hash = copyCurrent ? await temporaryFragment((await currentBundle()).bundle.docs[0]) : '';
    location.assign(`${base()}?temporary=1${copyCurrent ? '' : '&new=1'}${hash}`);
}

// Shows "Delete short link" when this browser holds an unexpired short link of the open document.
function renderMenu(tooLarge = false) {
    const owned = Object.values(readLinks()).filter((l) => l.nodeId === app.state.currentId);
    $('deleteShortLinkBtn').hidden = !owned.length;
    if (owned.length)
        $('deleteShortLinkLabel').textContent =
            `Delete short link${owned.length > 1 ? 's' : ''} · expires ${fmtExpiry(Math.max(...owned.map((l) => l.exp)))}`;
}

menu.addEventListener('toggle', async (e) => {
    if ((e as ToggleEvent).newState !== 'open') return;
    renderMenu();
});
$('copyLinkBtn').addEventListener('click', () => {
    hideMenu();
    if (app.state.currentId) void copyLinkDialog(app.state.currentId);
});
// The Share menu copies start the clipboard write in the click, with promises for the content
// (see `copyRich`). The HTML flavor escapes the name and the URL, because the name is document content.
$('copySlackBtn').addEventListener('click', async () => {
    hideMenu();
    const data = currentBundle().then(async ({ id, bundle }) => {
        const url = await bestLink(id, bundle);
        const open = splitComments(bundle.docs[0].text).threads.filter((t) => !t.resolved).length;
        const label = `${bundle.name} (MARS review${open ? `, ${open} open thread${open === 1 ? '' : 's'}` : ''})`;
        return { plain: `${label}: ${url}`, html: `<a href="${esc(url)}">${esc(label)}</a>` };
    });
    try {
        await copyRich(
            data.then((d) => d.plain),
            data.then((d) => d.html),
        );
        viewer.toast('Copied for Slack: paste it into a message');
    } catch (err) {
        viewer.toast((err as Error)?.message ?? 'Failed to copy');
    }
});
// One line, so it can sit at the end of a comment: GitHub picks the badge for its theme.
const githubBadge = (url: string) =>
    `<a href="${esc(url)}"><picture><source media="(prefers-color-scheme: dark)" srcset="${base()}badge-dark.svg"><img alt="View full review in MARS" src="${base()}badge.svg" height="28"></picture></a>`;
$('copyGithubBtn').addEventListener('click', async () => {
    hideMenu();
    const text = currentBundle().then(async ({ id, bundle }) => githubBadge(await bestLink(id, bundle)));
    try {
        await copyRich(text);
        viewer.toast('Copied a GitHub badge that links to this review');
    } catch (err) {
        viewer.toast((err as Error)?.message ?? 'Failed to copy');
    }
});
$('copyGithubThreadsBtn').addEventListener('click', async () => {
    hideMenu();
    const text = currentBundle().then(async ({ id, bundle }) =>
        githubSummary(bundle.name, await bestLink(id, bundle), splitComments(bundle.docs[0].text).threads),
    );
    try {
        await copyRich(text);
        viewer.toast('Copied a GitHub comment with the open threads');
    } catch (err) {
        viewer.toast((err as Error)?.message ?? 'Failed to copy');
    }
});
$('copyAgentBtn').addEventListener('click', async () => {
    hideMenu();
    const text = currentBundle().then(({ id, bundle }) => {
        const { threads } = splitComments(bundle.docs[0].text);
        if (!threads.some((t) => !t.resolved)) throw new Error('No open threads to hand off');
        return agentPrompt(app.node(id)!, threads);
    });
    try {
        await copyRich(text);
        viewer.toast('Copied the open threads for an AI agent: paste them into its chat');
    } catch (err) {
        viewer.toast((err as Error)?.message ?? 'Failed to copy');
    }
});
// Deletes every short link of the open document at the service. 404 counts as deleted. The
// registry is saved in `finally`, so a link that was deleted before a failure does not come back.
$('deleteShortLinkBtn').addEventListener('click', async () => {
    hideMenu();
    const links = readLinks();
    try {
        for (const [id, l] of Object.entries(links)) {
            if (l.nodeId !== app.state.currentId) continue;
            const res = await shortFetch(id, { method: 'DELETE', headers: { 'x-su-delete-token': l.token } });
            if (res.status !== 204 && res.status !== 404)
                throw new Error(`Could not delete the short link (HTTP ${res.status})`);
            delete links[id];
        }
        viewer.toast('Short link deleted');
    } catch (err) {
        viewer.toast((err as Error).message);
    } finally {
        writeLinks(links);
    }
});

// ---- Receiving ----
// A document without `originId` uses its own id as the origin.
const findDoc = (originId: string) =>
    app.state.nodes.find((n) => n.kind === 'doc' && (n.originId ?? n.id) === originId);
const sharedRoot = () => app.findOrCreateFolder(SHARED_FOLDER, null);

// Replaces the text of a stored document. A connected document goes through Sync, which can
// reject the text. The open document reloads in the viewer.
async function saveText(n: TreeNode, text: string) {
    if (n.connection) {
        await sync.snapshot(n, text);
        return;
    }
    Object.assign(n, { size: textBytes(text), threads: openThreads(text), updatedAt: Date.now() });
    await db.putNodes([n], { [n.id]: text });
    if (n.id === app.state.currentId)
        viewer.openDocument({
            text,
            name: n.name,
            handle: n.handle ?? null,
            stamp: n.stamp ?? 0,
            dirty: !!n.dirty,
            readingWidth: n.readingWidth,
        });
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? '' : 's'}`;

/**
 * Brings an incoming copy of a stored document in. Comments always merge. The text is
 * decided against `opened`, the source text as MARS last received it: when only the source
 * changed it is taken (and the changes marked), when only the stored copy changed it stays,
 * and when both changed (or the source text is unknown) the reader decides. `link` attaches
 * the file an incoming copy was opened from.
 *
 * The incoming text is untrusted. It is stored and merged as text only.
 * `readingWidth` is the width from the link. The call opens `existing` first, and it returns
 * without a change when the reader dismisses the dialog.
 */
async function updateExisting(
    existing: TreeNode,
    text: string,
    link?: { handle: FileSystemFileHandle; stamp: number },
    input?: InputOperation,
    readingWidth?: ReadingWidth,
) {
    checkInput(input);
    await viewer.flush();
    checkInput(input);
    await app.openDoc(existing.id, input);
    checkInput(input);
    const local = (await db.getText(existing.id)) ?? '';
    const opened = existing.opened ? await db.getSide('opened', existing.id) : undefined;
    checkInput(input);
    const merged = mergeDocuments(local, text);
    const comments = merged.added ? ` and merged ${plural(merged.added, 'new comment')}` : '';
    let choice: 'theirs' | 'mine' | 'copy' | '';
    let note: string;
    if (merged.sameBody || local === text) {
        choice = 'mine';
        note = merged.added
            ? `Merged ${plural(merged.added, 'new comment')} into "${existing.name}"`
            : `"${existing.name}" is already up to date`;
    } else if (opened !== undefined && app.bodyOf(opened) === app.bodyOf(text)) {
        choice = 'mine';
        note = `Kept your edits to "${existing.name}": its source hasn't changed${comments}`;
    } else if (opened !== undefined && app.bodyOf(opened) === app.bodyOf(local)) {
        choice = 'theirs';
        note = `Updated "${existing.name}"${comments} · showing what changed`;
    } else {
        const both = opened !== undefined;
        ({ value: choice } = (await modal({
            signal: input?.signal,
            title: both ? 'Both copies changed' : 'This document is already in your sidebar',
            html: both
                ? `<p>You edited <strong>${esc(existing.name)}</strong> in MARS, and the incoming copy changed too. Comments are merged in every case.</p><p><strong>Review changes</strong> takes the incoming text; your edits stay visible under <em>Show before</em> until you mark it reviewed.</p>`
                : `<p>The incoming copy of <strong>${esc(existing.name)}</strong> has different text than yours${merged.added ? ` and ${plural(merged.added, 'new comment')}` : ''}. Comments are merged in every case.</p><p><strong>Review changes</strong> takes the incoming text and marks what changed since your review.</p>`,
            buttons: both
                ? [
                      { label: 'Keep my text', value: 'mine' },
                      { label: 'Review changes', value: 'theirs' },
                      { label: 'Save as a copy', value: 'copy', primary: true },
                  ]
                : [
                      { label: 'Save as a copy', value: 'copy' },
                      { label: 'Keep my text', value: 'mine' },
                      { label: 'Review changes', value: 'theirs', primary: true },
                  ],
        })) as { value: typeof choice });
        note =
            choice === 'theirs' ? 'Showing what changed since your review' : 'Kept your text and merged the comments';
    }
    checkInput(input);
    if (choice === 'copy') {
        await app.createDoc({
            name: app.uniqueName(`${existing.name} (new)`, existing.parentId),
            text,
            parentId: existing.parentId,
            opened: true,
            handle: link?.handle,
            stamp: link?.stamp,
            input,
            readingWidth: readingWidth ?? existing.readingWidth,
        });
        checkInput(input);
        viewer.toast(`Saved the incoming copy next to "${existing.name}"`);
        return;
    }
    if (!choice) return;
    // The width from the link applies with the incoming text, or when the document has no shared width.
    if (validWidth(readingWidth) && (choice === 'theirs' || existing.readingWidth === undefined)) {
        existing.readingWidth = readingWidth;
        viewer.setDocumentReadingWidth(readingWidth);
    }
    if (link)
        Object.assign(existing, {
            handle: link.handle,
            stamp: link.stamp,
            dirty: choice === 'mine' && !merged.sameBody,
        });
    const next = choice === 'theirs' ? merged.theirs : merged.mine;
    if (next !== local) {
        // Sync can refuse the text of a connected document. Offer an independent local copy then, so
        // the incoming content is not lost.
        try {
            await saveText(existing, next);
        } catch (error) {
            if (!existing.connection) throw error;
            const answer = await modal({
                title: 'Keep the incoming copy?',
                body: 'This connection cannot accept all incoming edits or imported author identities. The connected document stays unchanged. Save an independent local copy to retain the incoming content.',
                buttons: [
                    { label: 'Cancel', value: '' },
                    { label: 'Save as a copy', value: 'copy', primary: true },
                ],
            });
            if (answer.value === 'copy')
                await app.createDoc({
                    name: app.uniqueName(`${existing.name} (incoming copy)`, null),
                    text,
                    parentId: null,
                    input,
                    readingWidth,
                });
            return;
        }
    } else await db.putNodes([existing]);
    await app.setOpened(existing, text);
    if (choice === 'theirs') await app.rebase(existing, local, merged.theirs);
    await db.putNodes([existing]);
    checkInput(input);
    viewer.toast(note);
}

/**
 * A stored document the incoming one is likely an updated copy of (a moved file, or a file
 * opened with Open), confirmed by the reader. The origin then follows the new location.
 * Returns null when no document scores above 0 and when the reader declines.
 * It reads the text of every stored document.
 */
async function findLikely(name: string, text: string, originId?: string, input?: InputOperation) {
    let best: TreeNode | null = null,
        bestScore = 0;
    for (const n of app.state.nodes) {
        if (n.kind !== 'doc') continue;
        const score = matchScore({ name, text }, { name: n.name, text: (await db.getText(n.id)) ?? '' });
        checkInput(input);
        if (score > bestScore) [best, bestScore] = [n, score];
    }
    if (!best) return null;
    const folder = ancestors(app.state.nodes, best.id).at(-1)?.name;
    const { value } = await modal({
        signal: input?.signal,
        title: 'Is this an updated copy?',
        html: `<p><strong>${esc(name)}</strong> looks like an updated copy of <strong>${esc(best.name)}</strong>${folder ? ` in <em>${esc(folder)}</em>` : ''}.</p><p>Updating it keeps one document with its comments and marks what changed.</p>`,
        buttons: [
            { label: 'Add as new', value: 'new' },
            { label: `Update "${best.name}"`, value: 'update', primary: true },
        ],
    });
    checkInput(input);
    if (value !== 'update') return null;
    // Only a location origin (`file:` or `url:`) replaces the stored origin.
    if (originId && /^(file|url):/.test(originId)) {
        best.originId = originId;
        await db.putNodes([best]);
    }
    return best;
}

// One shared document: update the stored document with the same origin or a confirmed likely
// copy. Otherwise add it to the default folder, or to "Shared with me".
async function receiveDoc(doc: Bundle['docs'][0], input?: InputOperation) {
    const existing = findDoc(doc.originId) ?? (await findLikely(doc.name, doc.text, doc.originId, input));
    checkInput(input);
    if (existing) {
        await updateExisting(existing, doc.text, undefined, input, doc.readingWidth);
        return;
    }
    const parent = app.defaultIncomingFolder() ?? (await sharedRoot());
    checkInput(input);
    await app.createDoc({
        name: app.uniqueName(doc.name || 'Untitled', parent.id),
        text: doc.text,
        parentId: parent.id,
        originId: doc.originId,
        opened: true,
        input,
        readingWidth: doc.readingWidth,
    });
    checkInput(input);
    viewer.toast(`Added "${doc.name}" to ${parent.name}`);
}

/**
 * A Markdown file opened with Open or dropped on the preview. Chromium hands over a handle,
 * which identifies a file already in the sidebar exactly; otherwise the content decides.
 *
 * `parentId`: undefined selects the default incoming folder, then the current folder.
 * A temporary session always adds a new top-level document and keeps no file handle.
 * A superseded `input` throws `AbortError`.
 */
export async function receiveFile(
    file: { name: string; text: string; handle: FileSystemFileHandle | null; stamp: number; parentId?: string | null },
    input: InputOperation = beginInput(),
) {
    await viewer.flush();
    input.check();
    if (db.isTemporaryStorage()) {
        await app.createDoc({
            name: app.uniqueName(file.name, null),
            text: file.text,
            parentId: null,
            opened: true,
            input,
        });
        return;
    }
    let existing: TreeNode | null = null;
    if (file.handle) {
        for (const n of app.state.nodes) {
            // `isSameEntry` compares the files on disk, not their names. A failed comparison counts as different.
            if (n.handle && (await n.handle.isSameEntry(file.handle).catch(() => false))) {
                existing = n;
                break;
            }
            input.check();
        }
    }
    input.check();
    existing ??= await findLikely(file.name, file.text, undefined, input);
    input.check();
    const link = file.handle ? { handle: file.handle, stamp: file.stamp } : undefined;
    if (existing) {
        await updateExisting(existing, file.text, link, input);
        return;
    }
    const parentId =
        file.parentId === undefined ? (app.defaultIncomingFolder()?.id ?? app.state.folderId) : file.parentId;
    await app.createDoc({
        name: app.uniqueName(file.name, parentId),
        text: file.text,
        parentId,
        handle: link?.handle,
        stamp: link?.stamp,
        opened: true,
        input,
    });
    input.check();
}

// A shared folder. A folder with the same origin receives the documents again. Otherwise a new
// folder goes to the default folder, or to "Shared with me".
async function receiveFolder(bundle: Bundle, input?: InputOperation) {
    let target = bundle.originId
        ? app.state.nodes.find((n) => n.kind === 'folder' && n.originId === bundle.originId)
        : undefined;
    if (!target) {
        const parent = app.defaultIncomingFolder() ?? (await sharedRoot());
        target = await app.createFolder(
            app.uniqueName(bundle.name || 'Untitled', parent.id),
            parent.id,
            bundle.originId,
        );
    }
    checkInput(input);
    let added = 0,
        merged = 0,
        differ = 0,
        first: string | null = null;
    for (const doc of bundle.docs) {
        checkInput(input);
        const existing = findDoc(doc.originId);
        // A known document keeps its text. A folder link merges comments only and never asks. The
        // toast reports the number of documents whose text differs.
        if (existing) {
            const addWidth = existing.readingWidth === undefined && validWidth(doc.readingWidth);
            if (addWidth) existing.readingWidth = doc.readingWidth;
            const local = (await db.getText(existing.id)) ?? '';
            checkInput(input);
            const m = mergeDocuments(local, doc.text);
            if (!m.sameBody) differ++;
            if (m.added) {
                merged += m.added;
                await saveText(existing, m.mine);
            } else if (addWidth) await db.putNodes([existing]);
            first ??= existing.id;
            continue;
        }
        let parentId = target.id;
        for (const seg of doc.path) parentId = (await app.findOrCreateFolder(seg, parentId)).id;
        checkInput(input);
        const created = await app.createDoc({
            name: app.uniqueName(doc.name, parentId),
            text: doc.text,
            parentId,
            originId: doc.originId,
            open: false,
            opened: true,
            input,
            readingWidth: doc.readingWidth,
        });
        first ??= created.id;
        added++;
    }
    await app.setOpen(target.id, true);
    checkInput(input);
    if (first) await app.openDoc(first, input);
    checkInput(input);
    const parts = [
        added && `added ${added} document${added === 1 ? '' : 's'}`,
        merged && `merged ${merged} comment${merged === 1 ? '' : 's'}`,
        differ && `kept your text in ${differ} changed document${differ === 1 ? '' : 's'}`,
    ].filter(Boolean);
    viewer.toast(`"${target.name}": ${parts.join(', ') || 'already up to date'}`);
}

/**
 * Brings a decoded bundle into the tree, after a save of pending edits. A bundle of one
 * document updates or adds that document. Every other bundle arrives as a folder.
 * A superseded `input` throws `AbortError`.
 */
export async function receive(bundle: Bundle, input: InputOperation = beginInput()) {
    await viewer.flush();
    input.check();
    if (bundle.kind === 'doc' && bundle.docs.length === 1) await receiveDoc(bundle.docs[0], input);
    else await receiveFolder(bundle, input);
}

/**
 * Fetches a remote Markdown file (?url=, or the empty-state URL box) into the tree.
 *
 * The browser fetches the address directly, without a proxy, so a cross-origin source needs CORS
 * headers. The response is untrusted text, and its origin id is `url:<address>`.
 * Throws an `Error` with a display message for an invalid address, an HTTP error, and a network
 * or CORS failure. A superseded `input` throws `AbortError`.
 */
export async function receiveUrl(url: string, input: InputOperation = beginInput()) {
    const cancelled = () => viewer.endLoading();
    input.signal.addEventListener('abort', cancelled, { once: true });
    try {
        const target = sourceUrl(url);
        await viewer.flush();
        input.check();
        const res = await fetch(target.href, { redirect: 'follow', signal: input.signal });
        input.check();
        if (!res.ok) throw new Error(`Fetch failed: HTTP ${res.status}`);
        const raw = await res.text();
        input.check();
        const { name, text } = readSource(raw, { fileName: urlFileName(target), fallback: 'document' });
        input.check();
        await receive(
            { v: 1, kind: 'doc', name, docs: [{ path: [], name, originId: `url:${target.href}`, text }] },
            input,
        );
        input.check();
        viewer.endLoading();
    } catch (error) {
        input.check();
        // `fetch` reports a CORS or network failure as a `TypeError`.
        if (error instanceof TypeError) throw new Error('Fetch failed (CORS or network). Try a raw, CORS-enabled URL.');
        throw error;
    } finally {
        input.signal.removeEventListener('abort', cancelled);
    }
}

/**
 * Opens whatever the address bar carries: `#doc=<id>`, a share link, or `?url=`.
 * Returns false when there was nothing to open.
 *
 * Share links: `#s=<secret>` (short), `#b=<state>` (folder), `#name=&md=&o=` or a bare
 * `#<state>` (document). Every one of them is untrusted input. True means that the address was
 * handled, also when the link was damaged and a message replaced it. The call starts an input
 * operation, so it supersedes a pending open.
 */
export async function openFromLocation(address = location.href): Promise<boolean> {
    const request = new URL(address);
    const hash = request.hash.slice(1);
    const params = new URLSearchParams(hash.includes('=') ? hash : '');
    const urlParam = request.searchParams.get('url');
    // A fragment of `t=1` alone marks an empty temporary session.
    if (params.get('t') === '1' && !params.has('md') && !params.has('s') && !params.has('b') && !urlParam) return false;
    if (!hash && !urlParam) return false;
    const input = beginInput();
    app.cancelTemporaryRoute();
    // Ends a failed open: clears the address, so a reload does not retry the link, and shows `msg`.
    // Null shows the notice of the viewer for a missing or expired short link.
    const fail = (msg: string | null) => {
        input.check();
        history.replaceState(history.state, '', location.pathname);
        viewer.endLoading(!msg);
        if (msg) viewer.toast(msg);
        return true;
    };
    try {
        // `#doc=<id>` is a route of this browser profile. An unknown id clears the address.
        if (params.has('doc')) {
            const id = params.get('doc')!;
            if (app.node(id)?.kind === 'doc') {
                await app.openDoc(id, input);
                return true;
            }
            history.replaceState(history.state, '', location.pathname);
            return false;
        }
        let bundle: Bundle | null = null;
        // Only the derived id goes to the service. The secret and the key stay in this page.
        if (params.has('s')) {
            const secret = params.get('s')!;
            let id: string;
            try {
                id = (await deriveShortLink(fromBase64Url(secret))).id;
            } catch {
                return fail(null);
            }
            const res = await shortFetch(id, { signal: input.signal });
            input.check();
            if (res.status === 404) return fail(null);
            if (!res.ok) return fail(`Could not load the short link (HTTP ${res.status}). Reload to retry.`);
            // The response is untrusted. `openSealed` authenticates the blob with the key from the
            // fragment and returns null for every other input.
            const { v, exp } = await res.json();
            bundle = await openSealed(secret, v);
            input.check();
            if (!bundle) return fail(null);
            if (exp) viewer.toast(`Shared link · expires ${fmtExpiry(exp)}`);
        } else if (params.has('b')) {
            const json = await decodeState(params.get('b')!);
            input.check();
            bundle = json === null ? null : parseBundle(json);
            if (!bundle) return fail('This link is damaged or incomplete.');
        } else if (hash) {
            // md.html-compatible full links: `#<state>` or `#name=<name>&md=<state>[&o=<originId>]`.
            const text = await decodeState(params.get('md') ?? (hash.includes('=') ? '' : hash));
            input.check();
            if (text === null) return fail('This link is damaged or incomplete.');
            const source = readSource(text, { name: params.get('name') ?? undefined });
            bundle = {
                v: 1,
                kind: 'doc',
                name: source.name,
                docs: [
                    {
                        path: [],
                        name: source.name,
                        originId: params.get('o') || `link:${await hashText(text)}`,
                        text: source.text,
                    },
                ],
            };
        } else if (urlParam) {
            try {
                await receiveUrl(urlParam, input);
            } catch (err) {
                if (!input.current()) return true;
                viewer.endLoading();
                viewer.toast((err as Error).message);
            }
            return true;
        } else {
            return false;
        }
        input.check();
        // A link from a temporary session must not enter the library. Reload on the temporary address.
        // The fragment stays, so the temporary session receives the same link.
        if ((params.get('t') === '1' || bundle.temporary) && !db.isTemporaryStorage()) {
            await viewer.flush();
            input.check();
            request.searchParams.set('temporary', '1');
            location.assign(request.href);
            return true;
        }
        // A fragment hint also supplies layout for older links whose bundle has no width.
        const readingWidth = params.get('w') === 'full' ? 'full' : Number(params.get('w'));
        if (validWidth(readingWidth)) for (const doc of bundle.docs) doc.readingWidth = readingWidth;
        await receive(bundle, input);
        return true;
    } catch (err) {
        if (!input.current()) return true;
        return fail(`Could not open the link: ${(err as Error)?.message ?? err}`);
    }
}
