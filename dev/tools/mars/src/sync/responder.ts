// Compiled into the existing MARS responder. It uses only the existing KV and SHA-256 ops.
// No waits, mutable shared head, native Web Crypto, Rust route, or SQL migration.
import { p256 } from '@noble/curves/nist.js';
import { canonical, checkpointReceipt, decode, encode, fail, jsonBytes, LIMITS, permitted, reverseSequence, SyncError, validCheckpoint, validGrant, validId,
    validInvitation, validWrappedKey, validBase64, delegatedGrant, type CollectionRoot, type WrappedKey, type SignedInvitation, type SignedCheckpoint, type SignedGrant } from './protocol.ts';

interface Entry { key: string; value?: string | null; createdAt?: number }
export interface Kv {
    get(key: string): Promise<string | null>;
    getEntry(key: string): Promise<{ value: string | null; expiresAt: number | null }>;
    set(key: string, value: string, options?: { ifAbsent?: boolean; ttlSec?: number }): Promise<void>;
    delete(key: string): Promise<boolean>;
    list(options: { prefix: string; after?: string; limit: number; valuesIncluded?: boolean }): Promise<{ entries: Entry[]; cursor: string | null }>;
}
export interface Runtime { kv: Kv; crypto: { sha256(bytes: string): Promise<string> | string } }
export interface Context { path: string; method: string; query?: Record<string, string>; headers?: Record<string, string>; body: Uint8Array; clientAddress?: string; secrets?: Record<string, string>; params?: { secrets?: Record<string, string> } }
export { prepareTestScope } from './testing.ts';
interface Collection { v: 1; owner: SignedGrant; root?: CollectionRoot }
interface Member { signed: SignedGrant; slot: string; keys?: WrappedKey[] }
interface InviteRecord { policy: SignedInvitation; tokenHash: string; inviter: string; bootstrap?: { nonce: string; ciphertext: string } }
interface Stored { envelope: SignedCheckpoint; author: SignedGrant; slots: string[]; payload?: string }
interface Permit { collection: string; operation: string; checkpoint: string; reservedAt: number; generation: number; busy: boolean }
const NS = 'sync/v1/';
const UNIT = 64 * 1024;
const COLLECTION_SLOTS = LIMITS.collectionBytes / UNIT;
const GLOBAL_SLOTS = 900 * 1024 * 1024 / UNIT; // Leave capacity for snapshots and metadata.
const clock = () => Math.floor(Date.now() / 1000);
const b64json = (value: unknown) => encode(jsonBytes(value));
const parse = <T>(value: string): T => JSON.parse(new TextDecoder().decode(decode(value)));
const signatureValid = (value: unknown, signature: string, publicKey: string) => {
    try { return p256.verify(decode(signature), jsonBytes(value), decode(publicKey), { prehash: true, lowS: false }); }
    catch { return false; }
};
const json = (statusCode: number, value?: unknown) => ({ statusCode, headers: { 'Content-Type': 'application/json; charset=utf-8',
    'Cache-Control': 'no-store', 'Referrer-Policy': 'no-referrer', 'X-Content-Type-Options': 'nosniff',
    'Cross-Origin-Resource-Policy': 'same-origin' }, body: value === undefined ? '' : JSON.stringify(value), skipRequest: true });
const grantKey = (collection: string, member: string) => `${NS}c/${collection}/members/${member}`;
const revokedKey = (collection: string, member: string) => `${NS}c/${collection}/revoked/${member}`;
const collectionKey = (collection: string) => `${NS}c/${collection}/owner`;
const docKey = (collection: string, document: string) => `${NS}c/${collection}/documents/${document}`;
const deletedKey = (collection: string, document: string) => `${NS}c/${collection}/deleted/${document}`;
const epochsPrefix = (collection: string, document: string) => `${NS}c/${collection}/epochs/${document}/`;
const inviteKey = (id: string) => `${NS}invitations/${id}`;
const revokedInviteKey = (collection: string, id: string) => `${NS}c/${collection}/revoked-invitations/${id}`;
/** Key writes per request. The client sends 25 for each request; each write costs up to six KV operations. */
const MAX_KEY_WRITES = 25;
/** base64url limits for small sealed values: an encrypted device name and an encrypted Live key update. */
const MAX_LABEL_CHARS = 1024, MAX_LIVE_KEY_CHARS = 512;
/** `touch` renews wrapped device keys only below this number of stream heads, to stay inside the KV operation budget. */
const KEY_RENEWAL_HEAD_LIMIT = 100;
const exactKeys = (value: unknown, keys: string[]): value is Record<string, any> => !!value && typeof value === 'object' && !Array.isArray(value)
    && Object.keys(value).sort().join(',') === keys.join(',');
/** An AES-GCM envelope as `sealJson` produces it: a 12-byte nonce and a bounded ciphertext. */
const validSealed = (value: unknown, maxChars: number) => exactKeys(value, ['ciphertext', 'nonce'])
    && validBase64(value.nonce, 16) && value.nonce.length === 16 && validBase64(value.ciphertext, maxChars) && value.ciphertext.length >= 22;
/** The current key epoch of a document. A document without a rotation is at epoch 1. */
async function currentEpoch(kv: Kv, collection: string, document: string) {
    const epochs = await kv.list({ prefix: epochsPrefix(collection, document), limit: 1, valuesIncluded: true });
    return epochs.entries[0]?.value ? parse<{ epoch: number }>(epochs.entries[0].value).epoch : 1;
}
async function liveKeyUpdate(kv: Kv, id: string, collection: string, document: string) {
    return read(kv, `${inviteKey(id)}/keys/${reverseSequence(await currentEpoch(kv, collection, document))}`);
}
const signedByOwner = (signed: SignedGrant, owner: SignedGrant) => signed.issuer
    ? delegatedGrant(signed, owner.grant) && signatureValid(signed.issuer.invitation, signed.issuer.signature, owner.grant.publicKey)
        && signatureValid(signed.grant, signed.signature, signed.issuer.invitation.publicKey)
    : signatureValid(signed.grant, signed.signature, owner.grant.publicKey);
const updatesPrefix = (collection: string, document = '') => `${NS}c/${collection}/updates/${document}${document ? '/' : ''}`;
const payloadKey = (checkpoint: string) => checkpoint.replace('/updates/', '/payloads/');
function takeCiphertext(checkpoint: SignedCheckpoint['checkpoint']) {
    const value = checkpoint.ciphertext; checkpoint.ciphertext = ''; return value;
}

async function first(kv: Kv, key: string, value: string, ttlSec: number | null = LIMITS.inactiveSeconds, crypto?: Runtime['crypto']) {
    const digest = crypto && value.length > 65536 ? await crypto.sha256(value) : null;
    await kv.set(key, value, { ifAbsent: true, ...(ttlSec === null ? {} : { ttlSec }) });
    if (digest && crypto) {
        // Drop the encoded write before reading it back. Native KV returns another
        // base64 string; retaining both exceeds the heap near the update-size cap.
        value = '';
        const stored = await kv.get(key);
        if (!stored || await crypto.sha256(stored) !== digest) return fail('immutable-record-conflict', 409);
    } else if (await kv.get(key) !== value) return fail('immutable-record-conflict', 409);
}
async function read<T>(kv: Kv, key: string): Promise<T | null> { const raw = await kv.get(key); return raw ? parse<T>(raw) : null; }
async function allKeys(kv: Kv, prefix: string, ceiling: number) {
    const entries: Entry[] = [];
    let after: string | undefined;
    do {
        const page = await kv.list({ prefix, after, limit: 1000, valuesIncluded: false });
        entries.push(...page.entries);
        if (entries.length > ceiling) return fail('index-limit', 507);
        if (page.entries.length < 1000) break;
        after = page.cursor ?? undefined;
        if (!after) return fail('invalid-storage-cursor', 503);
    } while (after);
    return entries;
}

// Each quota slot is a tiny immutable generation ledger, not a mutable head. A claimant
// competes for the next exact generation key with ifAbsent, then reads back the winner.
// Release appends a free generation. A delayed duplicate release cannot delete/release a
// later claimant's slot. Newest-first key scans do not depend on timestamps or notifications.
async function reserve(kv: Kv, collection: string, operation: string, bytes: number, checkpoint = ''): Promise<string[]> {
    const wanted = Math.ceil(bytes / UNIT);
    const result: string[] = [];
    const referencePrefix = `${NS}reservations/${collection}/${operation}/`;
    const references = await kv.list({ prefix: referencePrefix, limit: 64, valuesIncluded: true });
    const known = references.entries.flatMap((entry) => entry.value ? [parse<string>(entry.value)] : []);
    for (const [prefix, count] of [[`${NS}budget/all/`, GLOBAL_SLOTS], [`${NS}budget/${collection}/`, COLLECTION_SLOTS]] as const) {
        const owned: string[] = [];
        for (const key of known.filter((key) => key.startsWith(prefix))) {
            const slot = key.slice(0, key.lastIndexOf('/') + 1), head = (await kv.list({ prefix: slot, limit: 1, valuesIncluded: true })).entries[0];
            if (head?.key === key && head.value && parse<Permit>(head.value).operation === operation) {
                if (parse<Permit>(head.value).checkpoint !== checkpoint) return fail('operation-reused', 409);
                owned.push(key);
            }
        }
        // Bound collision work. Partial ownership is durable, so Retry makes progress
        // without a second allocation or an invocation beyond the KV operation budget.
        const start = (Number.parseInt(operation.slice(0, 8), 16) + known.length * 41) % count;
        for (let offset = 0; offset < 24 && owned.length < wanted; offset++) {
            const slot = `${prefix}${String((start + offset) % count).padStart(5, '0')}/`;
            const head = (await kv.list({ prefix: slot, limit: 1, valuesIncluded: true })).entries[0];
            const previous = head?.value ? parse<Permit>(head.value) : null;
            if (previous?.busy && previous.operation !== operation) continue;
            let key: string;
            if (previous?.busy) {
                if (previous.checkpoint !== checkpoint) return fail('operation-reused', 409);
                key = head.key;
            }
            else {
                const generation = (previous?.generation ?? 0) + 1;
                key = `${slot}${reverseSequence(generation)}`;
                const permit: Permit = { collection, operation, checkpoint, reservedAt: clock(), generation, busy: true };
                await kv.set(key, b64json(permit), { ifAbsent: true });
                const entry = await kv.getEntry(key);
                if (entry.expiresAt !== null) return fail('persistent-permits-unavailable', 503);
                if (!entry.value || parse<Permit>(entry.value).operation !== operation) continue;
            }
            if (owned.includes(key)) continue;
            await first(kv, referencePrefix + encode(new TextEncoder().encode(key)), b64json(key), null);
            owned.push(key);
        }
        if (owned.length !== wanted) return fail('sync-storage-full', 507);
        // A retry finds its slots through the reference listing, in another order than the first
        // attempt acquired them. Sort, so both attempts store the identical checkpoint record.
        result.push(...owned.sort());
    }
    return result;
}
async function release(kv: Kv, slots: string[], operation: string, maximum = 8, collection?: string) {
    const global = slots.filter((key) => key.startsWith(`${NS}budget/all/`)), local = slots.filter((key) => !key.startsWith(`${NS}budget/all/`));
    const interleaved = Array.from({ length: Math.max(global.length, local.length) }, (_, i) => [global[i], local[i]]).flat().filter((key): key is string => !!key);
    for (const key of interleaved.slice(0, maximum)) {
        const slot = key.slice(0, key.lastIndexOf('/') + 1), head = (await kv.list({ prefix: slot, limit: 1, valuesIncluded: true })).entries[0];
        const permit = head?.value ? parse<Permit>(head.value) : null;
        if (head?.key !== key || !permit?.busy || permit.operation !== operation) {
            const previousCollection = collection ?? (permit?.operation === operation ? permit.collection : undefined);
            if (previousCollection) await kv.delete(`${NS}reservations/${previousCollection}/${operation}/${encode(new TextEncoder().encode(key))}`);
            continue;
        }
        const generation = permit.generation + 1, next = `${slot}${reverseSequence(generation)}`;
        await kv.set(next, b64json({ ...permit, busy: false, generation }), { ifAbsent: true });
        await kv.delete(`${NS}reservations/${permit.collection}/${operation}/${encode(new TextEncoder().encode(key))}`);
        // Older generations can be pruned safely: no lower generation becomes current.
        const old = await kv.list({ prefix: slot, after: next, limit: 3, valuesIncluded: false });
        for (const entry of old.entries.slice(1)) await kv.delete(entry.key);
    }
}
async function allocate(kv: Kv, prefix: string, id: string, count: number, ttlSec = LIMITS.inactiveSeconds, limitError = 'membership-or-document-limit', limitStatus = 507) {
    const encoded = b64json(id);
    for (let i = 0; i < count; i++) {
        const key = `${prefix}${String(i).padStart(3, '0')}`;
        const existing = await kv.get(key);
        if (existing === encoded) return key;
        if (existing) continue;
        await kv.set(key, encoded, { ifAbsent: true, ttlSec });
        if (await kv.get(key) === encoded) return key;
    }
    return fail(limitError, limitStatus);
}

export async function handle(context: Context, runtime: Runtime) {
    if (!context.path.startsWith('/sync/')) return null;
    const { kv } = runtime;
    try {
        const action = context.path.slice('/sync/'.length), method = context.method.toUpperCase();
        if (action === 'limits' && method === 'GET') return json(200, { v: 1, ...LIMITS });
        if (method !== 'POST') return json(405, { error: 'method-not-allowed' });
        if (context.body.length > LIMITS.updateBytes) return json(413, { error: 'update-too-large' });
        const request = JSON.parse(new TextDecoder().decode(context.body));
        if (!request || typeof request !== 'object' || Array.isArray(request)) return fail('invalid-request');
        if (['invite-read', 'pair-request', 'pair-result', 'live-join'].includes(action)) {
            if (!validId(request.invitation)) return fail('invalid-invitation');
            const entry = await read<InviteRecord>(kv, inviteKey(request.invitation));
            const secret = context.headers?.authorization?.match(/^Bearer ([A-Za-z0-9_-]{43})$/)?.[1];
            if (!entry || entry.policy.invitation.expires <= clock()) return fail('invitation-expired', 410);
            if (!secret || await runtime.crypto.sha256(secret) !== entry.tokenHash) return fail('forbidden-invitation', 403);
            const p = entry.policy.invitation, collection = await read<Collection>(kv, collectionKey(p.collection));
            if (!collection || await kv.get(revokedKey(p.collection, entry.inviter)) || await kv.get(revokedInviteKey(p.collection, p.id))) return fail('connection-ended', 410);
            if (p.document && await kv.get(deletedKey(p.collection, p.document))) return fail('document-deleted', 410);
            if (!p.document && collection.root && await kv.get(deletedKey(p.collection, collection.root.document))) return fail('connection-ended', 410);
            if (action === 'invite-read') return json(200, { policy: entry.policy, owner: collection.owner, ...(entry.bootstrap ? { bootstrap: entry.bootstrap } : {}),
                root: p.document ? { document: p.document, kind: 'doc' } : collection.root,
                keyUpdate: p.mode === 'live' ? await liveKeyUpdate(kv, p.id, p.collection, p.document!) : null });
            if (action === 'live-join') {
                const signed = request.signed;
                if (p.mode !== 'live' || !validGrant(signed) || !signed.issuer || canonical(signed.issuer) !== canonical(entry.policy)
                    || !signedByOwner(signed, collection.owner)) return fail('invalid-live-grant', 403);
                const g = signed.grant;
                if (g.expires <= clock() || await kv.get(revokedKey(g.collection, g.member))) return fail('access-ended', 403);
                const slot = await allocate(kv, `${NS}c/${g.collection}/live-slots/`, g.member, LIMITS.liveGrants, Math.max(1, g.expires - clock()));
                await first(kv, grantKey(g.collection, g.member), b64json({ signed, slot }), Math.max(1, g.expires - clock()));
                return json(201, { member: g.member });
            }
            if (p.mode !== 'pair') return fail('invalid-invitation-mode');
            const pendingKey = `${inviteKey(p.id)}/request`;
            if (action === 'pair-request') {
                const candidate = request.candidate;
                if (!validGrant(candidate) || candidate.issuer || candidate.grant.collection !== p.collection
                    || candidate.grant.permission !== 'edit' || candidate.grant.document !== null
                    || candidate.grant.participant !== p.participant || candidate.grant.expires !== 0
                    || !signatureValid(candidate.grant, candidate.signature, candidate.grant.publicKey)) return fail('invalid-pair-request');
                // The label is the encrypted device name. Bound it: this endpoint needs no grant.
                const label = request.label ?? null;
                if (label !== null && !validSealed(label, MAX_LABEL_CHARS)) return fail('invalid-pair-request');
                await first(kv, pendingKey, b64json({ candidate, label }), Math.max(1, p.expires - clock()));
                return json(202, { requested: true });
            }
            const approved = await read<{ member: string }>(kv, `${inviteKey(p.id)}/approved`);
            if (!approved) return json(200, { approved: false });
            const member = await read<Member>(kv, grantKey(p.collection, approved.member));
            if (!member || await kv.get(revokedKey(p.collection, approved.member))) return fail('access-ended', 403);
            return json(200, { approved: true, ...member, owner: collection.owner, root: collection.root });
        }
        if (action === 'telemetry' || action === 'maintenance') {
            const secret = context.secrets?.MARS_STATS_API_KEY ?? context.params?.secrets?.MARS_STATS_API_KEY;
            const token = context.headers?.authorization?.replace(/^Bearer /, '');
            if (!secret) return fail('sync-telemetry-not-configured', 503);
            if (!token || await runtime.crypto.sha256(encode(new TextEncoder().encode(token))) !== await runtime.crypto.sha256(encode(new TextEncoder().encode(secret)))) return fail('forbidden', 403);
            const prefix = `${NS}budget/all/`;
            if (action === 'telemetry') {
                let after: string | undefined, currentSlot = '', reservedSlots = 0;
                const collections = new Set<string>();
                do {
                    const page = await kv.list({ prefix, after, limit: 1000, valuesIncluded: true });
                    for (const entry of page.entries) {
                        const slot = entry.key.slice(0, entry.key.lastIndexOf('/'));
                        if (slot === currentSlot) continue;
                        currentSlot = slot;
                        const permit = entry.value ? parse<Permit>(entry.value) : null;
                        if (permit?.busy) { reservedSlots++; collections.add(permit.collection); }
                    }
                    if (page.entries.length < 1000) break;
                    after = page.cursor ?? undefined;
                    if (!after) return fail('invalid-storage-cursor', 503);
                } while (after);
                // Reserved capacity is not actual stored bytes or an active-reader count.
                // No document, collection, participant, or credential identifiers leave this endpoint.
                return json(200, { reservedBytes: reservedSlots * UNIT, maximumReservedBytes: GLOBAL_SLOTS * UNIT,
                    collectionsWithReservations: collections.size, reservedSlots, unitBytes: UNIT });
            }
            if (request.after && (typeof request.after !== 'string' || !request.after.startsWith(prefix) || request.after.length > 256)) return fail('invalid-maintenance-cursor');
            const page = await kv.list({ prefix, after: request.after || undefined, limit: 8, valuesIncluded: true });
            let reclaimedOperations = 0, currentSlot = '', cursor = page.cursor;
            for (const entry of page.entries) {
                const slot = entry.key.slice(0, entry.key.lastIndexOf('/') + 1);
                if (slot === currentSlot) continue;
                currentSlot = slot;
                const head = (await kv.list({ prefix: slot, limit: 1, valuesIncluded: true })).entries[0];
                const permit = head?.value ? parse<Permit>(head.value) : null;
                // Abandoned reservations receive a two-day grace period. Ordinary request
                // execution is bounded to seconds. No in-flight upload is reclaimed here.
                if (!permit?.busy || !permit.checkpoint) continue;
                const record = await kv.get(permit.checkpoint);
                const document = permit.checkpoint.split('/')[5];
                const deleted = validId(document) && !!await kv.get(deletedKey(permit.collection, document));
                if (record) {
                    const prefix = permit.checkpoint.slice(0, permit.checkpoint.lastIndexOf('/') + 1);
                    const newest = await kv.list({ prefix, limit: 2, valuesIncluded: false });
                    if (!deleted && (newest.entries.length < 2 || newest.entries.some(({ key }) => key === permit.checkpoint))) continue;
                    await kv.delete(permit.checkpoint);
                } else if (!deleted && permit.reservedAt > clock() - 2 * 86400 && !(await kv.get(`${NS}c/${permit.collection}/receipts/${permit.operation}`))) continue;
                await kv.delete(payloadKey(permit.checkpoint));
                const refs = await kv.list({ prefix: `${NS}reservations/${permit.collection}/${permit.operation}/`, limit: 64, valuesIncluded: true });
                await release(kv, refs.entries.flatMap((ref) => ref.value ? [parse<string>(ref.value)] : []), permit.operation, 8, permit.collection);
                reclaimedOperations++;
                cursor = entry.key;
                break; // One potentially large reservation per bounded request.
            }
            return json(200, { reclaimedOperations, cursor, complete: !reclaimedOperations && page.entries.length < 8 });
        }
        if (action === 'create') {
            const signed = request.owner;
            if (!validGrant(signed) || signed.grant.permission !== 'owner' || signed.grant.document !== null || signed.grant.expires !== 0
                || !signatureValid(signed.grant, signed.signature, signed.grant.publicKey)) return fail('invalid-owner');
            const g = signed.grant, ip = String(context.clientAddress ?? 'unknown').replace(/:\d+$/, '');
            const ipHash = await runtime.crypto.sha256(encode(new TextEncoder().encode(ip)));
            await allocate(kv, `${NS}create/${ipHash}/${Math.floor(clock() / 3600)}/`, g.collection, LIMITS.collections, 3600, 'collection-creation-limit', 429);
            if (signed.issuer) return fail('invalid-owner');
            const root = request.root;
            if (root && (!validId(root.document) || !['doc', 'folder'].includes(root.kind))) return fail('invalid-root');
            const collection: Collection = { v: 1, owner: signed, ...(root ? { root: { document: root.document, kind: root.kind } } : {}) };
            await first(kv, collectionKey(g.collection), b64json(collection));
            const slot = await allocate(kv, `${NS}c/${g.collection}/member-slots/`, g.member, LIMITS.members);
            await first(kv, grantKey(g.collection, g.member), b64json({ signed, slot }));
            return json(201, { collection: g.collection, limits: LIMITS });
        }
        if (!validId(request.collection) || !validId(request.member)) return fail('invalid-scope');
        const collection = await read<Collection>(kv, collectionKey(request.collection));
        if (!collection) return fail('connection-expired', 410);
        let member = await read<Member>(kv, grantKey(request.collection, request.member));
        const token = context.headers?.authorization?.match(/^Bearer ([A-Za-z0-9_-]{43})$/)?.[1];
        if (!member && token && request.member === collection.owner.grant.member
            && await runtime.crypto.sha256(token) === collection.owner.grant.tokenHash) {
            const slot = await allocate(kv, `${NS}c/${request.collection}/member-slots/`, request.member, LIMITS.members);
            member = { signed: collection.owner, slot };
            await first(kv, grantKey(request.collection, request.member), b64json(member));
        }
        if (!member || !token || await kv.get(revokedKey(request.collection, request.member)) || await runtime.crypto.sha256(token) !== member.signed.grant.tokenHash
            || !signedByOwner(member.signed, collection.owner) || await kv.get(member.slot) !== b64json(request.member)) return fail('access-ended', 403);
        const grant = member.signed.grant;
        if (grant.expires && grant.expires <= clock()) return fail('access-expired', 403);
        if (member.signed.issuer) {
            // Invitation ids share one namespace across collections. Require the record of exactly
            // this invitation, and no revocation fence: another collection can create a record
            // with a revoked id, which must not restore the access of this member.
            const invitation = member.signed.issuer.invitation.id, invite = await read<InviteRecord>(kv, inviteKey(invitation));
            if (!invite || canonical(invite.policy) !== canonical(member.signed.issuer)
                || await kv.get(revokedInviteKey(request.collection, invitation))) return fail('live-link-ended', 403);
        }
        const scope = (id: unknown) => {
            if (!validId(id) || (grant.document !== null && grant.document !== id)) return fail('forbidden-document', 403);
            return id;
        };
        const owner = () => { if (grant.permission !== 'owner') return fail('owner-required', 403); };
        const readDirectory = async (deleted = new Map<string, boolean>()) => {
            if (grant.document !== null) return fail('forbidden-directory', 403);
            const page = await kv.list({ prefix: `${NS}c/${grant.collection}/documents/`, limit: LIMITS.documents, valuesIncluded: true });
            const documents = [];
            for (const entry of page.entries) if (entry.value) {
                const doc = parse<{ id: string; kind: string; creator?: string }>(entry.value);
                if (!deleted.has(doc.id)) deleted.set(doc.id, !!await kv.get(deletedKey(grant.collection, doc.id)));
                documents.push({ id: doc.id, kind: doc.kind, creator: doc.creator ?? collection.owner.grant.member, deleted: deleted.get(doc.id)! });
            }
            return { documents, root: collection.root };
        };
        if (action !== 'purge-live-document' && collection.root?.kind === 'doc' && await kv.get(deletedKey(grant.collection, collection.root.document))) return fail('connection-ended', 410);
        const wrappedKeys = (signed: SignedGrant, values: unknown): WrappedKey[] => {
            if (values === undefined) return [];
            if (!Array.isArray(values) || values.length > LIMITS.documents || !values.every(validWrappedKey)) return fail('invalid-key-envelopes');
            if (values.some((key) => key.collection !== grant.collection || key.member !== signed.grant.member
                || key.sender !== collection.owner.grant.boxKey || (signed.grant.document !== null && key.document !== signed.grant.document))) return fail('forbidden-key-envelope', 403);
            return values;
        };
        if (action === 'bootstrap') return json(200, { signed: member.signed, keys: member.keys ?? [], owner: collection.owner,
            root: grant.document ? { document: grant.document, kind: 'doc' } : collection.root,
            keyUpdate: member.signed.issuer ? await liveKeyUpdate(kv, member.signed.issuer.invitation.id, grant.collection, grant.document!) : null });
        if (action === 'members') {
            owner(); const entries = await kv.list({ prefix: `${NS}c/${grant.collection}/members/`, limit: LIMITS.members + LIMITS.liveGrants, valuesIncluded: true });
            return json(200, { members: entries.entries.flatMap((entry) => entry.value ? [parse<Member>(entry.value).signed] : []) });
        }
        if (action === 'key-write') {
            owner(); const values = request.values;
            if (!Array.isArray(values) || values.length > MAX_KEY_WRITES) return fail('invalid-key-update');
            const written: string[] = [], members = new Map<string, boolean>();
            for (const value of values) {
                // Exact shape: an extra field would store unmetered bytes beside the small envelope.
                if (!exactKeys(value, ['signature', 'wrapped']) || !validBase64(value.signature, 86)
                    || !validWrappedKey(value.wrapped) || value.wrapped.collection !== grant.collection || value.wrapped.sender !== collection.owner.grant.boxKey
                    || !signatureValid(value.wrapped, value.signature, collection.owner.grant.publicKey)) return fail('invalid-key-signature', 403);
                // A key for a member without a grant has no reader. Skip it, so key records stay
                // bounded by the membership. An expired member in a saved rotation plan is not an error.
                const target = value.wrapped.member;
                if (!members.has(target)) members.set(target, !!await kv.get(grantKey(grant.collection, target)));
                if (!members.get(target)) continue;
                const key = `${NS}c/${grant.collection}/keys/${value.wrapped.member}/${value.wrapped.document}/${reverseSequence(value.wrapped.epoch)}`;
                await first(kv, key, b64json(value));
                written.push(key);
                const older = await kv.list({ prefix: key.slice(0, key.lastIndexOf('/') + 1), after: key, limit: 3, valuesIncluded: false });
                for (const entry of older.entries.slice(1)) await kv.delete(entry.key);
            }
            if (collection.root?.kind === 'doc' && await kv.get(deletedKey(grant.collection, collection.root.document))) {
                for (const key of written) await kv.delete(key);
                return fail('connection-ended', 410);
            }
            return json(200, { written: values.length });
        }
        if (action === 'key-read') {
            const id = scope(request.document), prefix = `${NS}c/${grant.collection}/keys/${grant.member}/${id}/`;
            const page = await kv.list({ prefix, limit: 2, valuesIncluded: true });
            return json(200, { values: page.entries.flatMap((entry) => entry.value ? [parse(entry.value)] : []) });
        }
        if (action === 'epoch') {
            const id = scope(request.document), page = await kv.list({ prefix: epochsPrefix(grant.collection, id), limit: 1, valuesIncluded: true });
            return json(200, { epoch: page.entries[0]?.value ? parse<{ epoch: number }>(page.entries[0].value).epoch : 1 });
        }
        if (action === 'rotate-document') {
            owner(); const id = scope(request.document), epoch = request.epoch;
            const page = await kv.list({ prefix: epochsPrefix(grant.collection, id), limit: 1, valuesIncluded: true });
            const current = page.entries[0]?.value ? parse<{ epoch: number }>(page.entries[0].value).epoch : 1;
            if (!Number.isSafeInteger(epoch) || (epoch !== current && epoch !== current + 1)) return fail('invalid-key-epoch', 409);
            if (epoch === current) return json(200, { epoch });
            await first(kv, epochsPrefix(grant.collection, id) + reverseSequence(epoch), b64json({ epoch }));
            return json(200, { epoch });
        }
        if (action === 'live-key-write') {
            owner(); if (!validId(request.invitation)) return fail('invalid-invitation');
            const invite = await read<InviteRecord>(kv, inviteKey(request.invitation));
            if (!invite || invite.policy.invitation.collection !== grant.collection || invite.policy.invitation.mode !== 'live') return fail('forbidden-invitation', 403);
            const value = request.value;
            if (!exactKeys(value, ['ciphertext', 'document', 'epoch', 'nonce']) || !validSealed({ nonce: value.nonce, ciphertext: value.ciphertext }, MAX_LIVE_KEY_CHARS)
                || !validBase64(request.signature, 86) || value.document !== invite.policy.invitation.document
                || !signatureValid(value, request.signature, collection.owner.grant.publicKey)) return fail('invalid-key-signature', 403);
            // A key update exists for the current epoch or for the next one, which a rotation activates afterwards.
            if (!Number.isSafeInteger(value.epoch) || value.epoch < 1 || value.epoch > await currentEpoch(kv, grant.collection, value.document) + 1) return fail('invalid-key-epoch');
            const key = `${inviteKey(request.invitation)}/keys/${reverseSequence(value.epoch)}`;
            await first(kv, key, b64json({ value, signature: request.signature }), Math.max(1, invite.policy.invitation.expires - clock()));
            if (await kv.get(deletedKey(grant.collection, value.document))) { await kv.delete(key); return fail('document-deleted', 410); }
            return json(200, { updated: true });
        }
        if (action === 'invite') {
            owner();
            const policy = request.policy;
            if (!validInvitation(policy) || policy.invitation.collection !== grant.collection
                || !signatureValid(policy.invitation, policy.signature, collection.owner.grant.publicKey)) return fail('invalid-invitation');
            const p = policy.invitation;
            if (await kv.get(revokedInviteKey(grant.collection, p.id))) return fail('invitation-revoked', 403);
            if (p.expires <= clock() || p.expires > clock() + (p.mode === 'pair' ? LIMITS.invitationSeconds : LIMITS.maxLiveSeconds)
                || (p.mode === 'pair' && p.participant !== collection.owner.grant.participant)) return fail('invalid-invitation-lifetime');
            if (p.document) {
                const doc = await read<{ kind: string }>(kv, docKey(grant.collection, p.document));
                if (!doc || doc.kind !== 'doc' || await kv.get(deletedKey(grant.collection, p.document))) return fail('invalid-live-document');
            }
            if (typeof request.tokenHash !== 'string' || !/^[a-f0-9]{64}$/.test(request.tokenHash)) return fail('invalid-invitation-token');
            const bootstrap = request.bootstrap;
            if (bootstrap !== undefined && (!bootstrap || typeof bootstrap !== 'object'
                || Object.keys(bootstrap).sort().join(',') !== 'ciphertext,nonce' || !validBase64(bootstrap.nonce, 16) || bootstrap.nonce.length !== 16
                || !validBase64(bootstrap.ciphertext, 16384) || bootstrap.ciphertext.length < 22)) return fail('invalid-live-bootstrap');
            // Invitation ids share one namespace. Refuse an id that holds a different invitation before
            // a slot points at it: cleanup of this collection follows its slots.
            const existing = await read<InviteRecord>(kv, inviteKey(p.id));
            if (existing && canonical(existing.policy) !== canonical(policy)) return fail('immutable-record-conflict', 409);
            await allocate(kv, `${NS}c/${grant.collection}/invitation-slots/`, p.id, LIMITS.liveGrants, Math.max(1, p.expires - clock()));
            await first(kv, inviteKey(p.id), b64json({ policy, tokenHash: request.tokenHash, inviter: grant.member, ...(bootstrap ? { bootstrap } : {}) }), Math.max(1, p.expires - clock()));
            return json(201, { invitation: p.id });
        }
        if (action === 'pair-pending' || action === 'pair-approve') {
            owner();
            if (!validId(request.invitation)) return fail('invalid-invitation');
            const entry = await read<InviteRecord>(kv, inviteKey(request.invitation));
            if (!entry || entry.policy.invitation.collection !== grant.collection || entry.policy.invitation.mode !== 'pair'
                || entry.policy.invitation.expires <= clock()) return fail('invitation-expired', 410);
            const pending = await read<{ candidate: SignedGrant; label: unknown }>(kv, `${inviteKey(request.invitation)}/request`);
            if (action === 'pair-pending') return json(200, pending ?? { candidate: null });
            const signed = request.signed;
            if (!pending || !validGrant(signed) || signed.issuer || canonical(signed.grant) !== canonical(pending.candidate.grant)
                || !signedByOwner(signed, collection.owner)) return fail('invalid-pair-approval', 403);
            const keys = wrappedKeys(signed, request.keys), g = signed.grant;
            if (await kv.get(revokedKey(grant.collection, g.member))) return fail('member-revoked', 403);
            const slot = await allocate(kv, `${NS}c/${grant.collection}/member-slots/`, g.member, LIMITS.members);
            await first(kv, grantKey(grant.collection, g.member), b64json({ signed, slot, keys }));
            await first(kv, `${inviteKey(request.invitation)}/approved`, b64json({ member: g.member }), Math.max(1, entry.policy.invitation.expires - clock()));
            return json(200, { approved: true });
        }
        if (action === 'invite-revoke') {
            owner(); if (!validId(request.invitation)) return fail('invalid-invitation');
            if (await kv.get(revokedInviteKey(grant.collection, request.invitation))) return json(200, { revoked: true });
            const entry = await read<InviteRecord>(kv, inviteKey(request.invitation));
            if (!entry || entry.policy.invitation.collection !== grant.collection) return fail('forbidden-invitation', 403);
            await first(kv, revokedInviteKey(grant.collection, request.invitation), b64json({ revoked: true }), null);
            await kv.delete(inviteKey(request.invitation)); return json(200, { revoked: true });
        }
        if (action === 'directory') return json(200, await readDirectory());
        if (action === 'delete-document') {
            owner(); const id = scope(request.document);
            // The fence never expires. Write it only for a registered document, so requests with
            // random ids cannot add permanent records.
            if (await kv.get(docKey(grant.collection, id)) || await kv.get(deletedKey(grant.collection, id))) await first(kv, deletedKey(grant.collection, id), b64json({ deleted: true }), null);
            return json(200, { deleted: id });
        }
        if (action === 'purge-live-document') {
            owner(); const id = scope(request.document);
            if (collection.root?.document !== id || collection.root.kind !== 'doc') return fail('personal-sync-active', 409);
            const started = !!await kv.get(deletedKey(grant.collection, id));
            const members = await kv.list({ prefix: `${NS}c/${grant.collection}/members/`, limit: LIMITS.members + LIMITS.liveGrants, valuesIncluded: true });
            if (!started && members.entries.some(entry => entry.value && parse<Member>(entry.value).signed.grant.member !== grant.member
                && parse<Member>(entry.value).signed.grant.document === null)) return fail('personal-sync-active', 409);
            const documents = await kv.list({ prefix: `${NS}c/${grant.collection}/documents/`, limit: LIMITS.documents, valuesIncluded: true });
            if (!started && documents.entries.some(entry => entry.value && parse<{ id: string }>(entry.value).id !== id)) return fail('personal-sync-active', 409);
            const invitations = await kv.list({ prefix: `${NS}c/${grant.collection}/invitation-slots/`, limit: LIMITS.liveGrants, valuesIncluded: true });
            for (const slot of invitations.entries) if (slot.value) {
                const invitation = await read<InviteRecord>(kv, inviteKey(parse<string>(slot.value)));
                if (!started && invitation?.policy.invitation.mode === 'pair') return fail('personal-sync-active', 409);
            }
            // The irreversible fence blocks reads, joins, retries, and new writes.
            // Cleanup is bounded and repeatable, including after a lost response.
            await first(kv, deletedKey(grant.collection, id), b64json({ deleted: true }), null);
            const updates = await kv.list({ prefix: updatesPrefix(grant.collection, id), limit: 1, valuesIncluded: true });
            if (updates.entries[0]?.value) {
                const entry = updates.entries[0], stored = parse<Stored>(entry.value!);
                if (stored.payload) await kv.delete(stored.payload);
                const refs = await kv.list({ prefix: `${NS}reservations/${grant.collection}/${stored.envelope.checkpoint.operation}/`, limit: 8, valuesIncluded: true });
                await release(kv, refs.entries.flatMap(ref => ref.value ? [parse<string>(ref.value)] : []), stored.envelope.checkpoint.operation, 8, grant.collection);
                if (!(await kv.list({ prefix: `${NS}reservations/${grant.collection}/${stored.envelope.checkpoint.operation}/`, limit: 1 })).entries.length) await kv.delete(entry.key);
                return json(200, { complete: false });
            }
            const refs = await kv.list({ prefix: `${NS}reservations/${grant.collection}/`, limit: 8, valuesIncluded: true });
            if (refs.entries.length) {
                const operations = new Set(refs.entries.map(ref => ref.key.split('/')[4]));
                for (const operation of operations) await release(kv, refs.entries.filter(ref => ref.key.split('/')[4] === operation).flatMap(ref => ref.value ? [parse<string>(ref.value)] : []), operation, 8, grant.collection);
                return json(200, { complete: false });
            }
            if (invitations.entries.length) {
                const slot = invitations.entries[0], invitation = slot.value ? parse<string>(slot.value) : null;
                // A slot can name an id that another collection holds. Delete only the records of an
                // invitation of this collection, or the leftovers of one whose main record is gone.
                const record = invitation ? await read<InviteRecord>(kv, inviteKey(invitation)) : null;
                const owned = !record || record.policy.invitation.collection === grant.collection;
                const entries = invitation && owned ? await kv.list({ prefix: inviteKey(invitation), limit: 16 }) : { entries: [] };
                for (const entry of entries.entries) await kv.delete(entry.key);
                if (entries.entries.length < 16) await kv.delete(slot.key);
                return json(200, { complete: false });
            }
            const budget = await kv.list({ prefix: `${NS}budget/${grant.collection}/`, limit: 16 });
            if (budget.entries.length) { for (const entry of budget.entries) await kv.delete(entry.key); return json(200, { complete: false }); }
            const prefix = `${NS}c/${grant.collection}/`;
            if (request.after && (typeof request.after !== 'string' || request.after.length > 256 || !request.after.startsWith(prefix))) return fail('invalid-cleanup-cursor');
            const page = await kv.list({ prefix, after: request.after || undefined, limit: 32 });
            const protectedKeys = new Set([collectionKey(grant.collection), grantKey(grant.collection, grant.member), member.slot]);
            for (const entry of page.entries) if (!protectedKeys.has(entry.key) && !entry.key.includes('/revoked/')
                && !entry.key.includes('/revoked-invitations/') && !entry.key.includes('/deleted/')) await kv.delete(entry.key);
            if (page.entries.length === 32) return json(200, { complete: false, after: page.cursor });
            await kv.delete(member.slot); await kv.delete(grantKey(grant.collection, grant.member)); await kv.delete(collectionKey(grant.collection));
            return json(200, { complete: true });
        }

        if (action === 'grant') {
            owner();
            const signed = request.signed;
            if (!validGrant(signed) || signed.grant.collection !== grant.collection || signed.grant.permission === 'owner'
                || !signatureValid(signed.grant, signed.signature, collection.owner.grant.publicKey)) return fail('invalid-grant');
            const g = signed.grant;
            if (await kv.get(revokedKey(grant.collection, g.member))) return fail('member-revoked', 403);
            if (g.document !== null) {
                scope(g.document);
                if (g.expires <= clock() || g.expires > clock() + LIMITS.maxLiveSeconds) return fail('invalid-grant-lifetime');
            } else if (g.expires !== 0) return fail('invalid-member-lifetime');
            const prefix = `${NS}c/${grant.collection}/${g.document === null ? 'member' : 'live'}-slots/`;
            const slot = await allocate(kv, prefix, g.member, g.document === null ? LIMITS.members : LIMITS.liveGrants);
            const keys = wrappedKeys(signed, request.keys);
            await first(kv, grantKey(grant.collection, g.member), b64json({ signed, slot, ...(keys.length ? { keys } : {}) }));
            return json(201, { member: g.member });
        }
        if (action === 'revoke') {
            owner();
            if (!validId(request.target) || request.target === collection.owner.grant.member) return fail('invalid-revocation');
            // An immutable fence prevents a late activity request from restoring access.
            // Keep this small, opaque identity fence until maintenance proves all grants expired.
            // The fence never expires. Write it only for a member that has, or had, a grant here, so
            // requests with random ids cannot add permanent records.
            const target = await read<Member>(kv, grantKey(grant.collection, request.target));
            if (!target && !await kv.get(revokedKey(grant.collection, request.target))) return json(200, { revoked: request.target });
            await kv.set(revokedKey(grant.collection, request.target), b64json({ at: clock() }), { ifAbsent: true });
            if ((await kv.getEntry(revokedKey(grant.collection, request.target))).expiresAt !== null) return fail('persistent-revocation-unavailable', 503);
            await kv.delete(grantKey(grant.collection, request.target));
            // Free the slot. A revoked Live member otherwise holds one of the collection's slots until its grant expires.
            if (target && await kv.get(target.slot) === b64json(request.target)) await kv.delete(target.slot);
            return json(200, { revoked: request.target });
        }
        if (action === 'document') {
            const id = scope(request.document);
            if (await kv.get(deletedKey(grant.collection, id))) return fail('document-deleted', 410);
            if (!permitted(grant, id, 'tree') || !['doc', 'folder'].includes(request.kind)) return fail('forbidden-document-create', 403);
            const slot = await allocate(kv, `${NS}c/${grant.collection}/document-slots/`, id, LIMITS.documents);
            await first(kv, docKey(grant.collection, id), b64json({ id, kind: request.kind, slot, creator: grant.member }));
            return json(201, { document: id });
        }
        if (action === 'document-info') {
            const id = scope(request.document);
            if (await kv.get(deletedKey(grant.collection, id))) return fail('document-deleted', 410);
            const doc = await read<{ kind: string; creator?: string }>(kv, docKey(grant.collection, id));
            if (!doc) return fail('document-expired', 410);
            return json(200, { document: id, kind: doc.kind, creator: doc.creator ?? collection.owner.grant.member });
        }
        if (action === 'index') {
            if (collection.root && await kv.get(deletedKey(grant.collection, collection.root.document))) return fail('connection-ended', 410);
            if ((grant.document || request.document) && await kv.get(deletedKey(grant.collection, scope(grant.document ?? request.document)))) return fail('document-deleted', 410);
            const prefix = updatesPrefix(grant.collection, grant.document ?? (request.document ? scope(request.document) : ''));
            if (request.after && (typeof request.after !== 'string' || !request.after.startsWith(prefix) || request.after.length > 256)) return fail('invalid-cursor');
            const page = await kv.list({ prefix, after: request.after || undefined, limit: LIMITS.pageSize, valuesIncluded: false });
            const deleted = new Map<string, boolean>(), entries = [];
            if (request.exclude && (!Array.isArray(request.exclude) || request.exclude.length > LIMITS.documents || !request.exclude.every(validId))) return fail('invalid-exclusions');
            const excluded = new Set<string>(request.exclude ?? []);
            for (const { key } of page.entries) {
                const document = key.split('/')[5];
                if (excluded.has(document)) continue;
                if (!deleted.has(document)) deleted.set(document, !!await kv.get(deletedKey(grant.collection, document)));
                if (!deleted.get(document)) entries.push({ key });
            }
            return json(200, { entries, cursor: page.cursor,
                complete: page.entries.length < LIMITS.pageSize, owner: collection.owner,
                ...(request.directory === true ? { directory: await readDirectory(deleted) } : {}) });
        }
        if (action === 'get') {
            const prefix = updatesPrefix(grant.collection, grant.document ?? '');
            if (typeof request.key !== 'string' || !request.key.startsWith(prefix) || request.key.length > 256) return fail('forbidden-checkpoint', 403);
            const stored = await read<Stored>(kv, request.key);
            if (stored && await kv.get(deletedKey(grant.collection, stored.envelope.checkpoint.document))) return fail('document-deleted', 410);
            if (stored?.payload) {
                const ciphertext = await kv.get(stored.payload);
                if (!ciphertext) return json(404, { error: 'checkpoint-unavailable' });
                stored.envelope.checkpoint.ciphertext = ciphertext;
            }
            return stored ? json(200, { envelope: stored.envelope, author: stored.author, owner: collection.owner }) : json(404, { error: 'checkpoint-unavailable' });
        }
        if (action === 'append') {
            const envelope = request.envelope;
            if (!validCheckpoint(envelope)) return fail('invalid-checkpoint');
            const c = envelope.checkpoint;
            scope(c.document);
            if (await kv.get(deletedKey(grant.collection, c.document))) return fail('document-deleted', 410);
            const epoch = await kv.list({ prefix: epochsPrefix(grant.collection, c.document), limit: 1, valuesIncluded: true });
            const currentEpoch = epoch.entries[0]?.value ? parse<{ epoch: number }>(epoch.entries[0].value).epoch : 1;
            if (c.epoch !== currentEpoch) return fail('key-epoch-changed', 409);
            if (c.collection !== grant.collection || c.member !== grant.member || !permitted(grant, c.document, c.stream)) return fail('forbidden-stream', 403);
            if (!signatureValid(c, envelope.signature, grant.publicKey)) return fail('invalid-signature', 403);
            if (!(await kv.get(docKey(grant.collection, c.document)))) return fail('document-expired', 410);
            // Bind every signed field and the ciphertext through a domain-separated
            // transcript hash, without another full-size base64 copy of the envelope.
            const ciphertextHash = await runtime.crypto.sha256(c.ciphertext);
            const digest = await runtime.crypto.sha256(b64json(checkpointReceipt(envelope, ciphertextHash)));
            const bytes = c.ciphertext.length + jsonBytes({ ...envelope, checkpoint: { ...c, ciphertext: '' } }).length;
            const receiptKey = `${NS}c/${grant.collection}/receipts/${c.operation}`;
            const prior = await read<{ digest: string }>(kv, receiptKey);
            if (prior) {
                if (prior.digest !== digest) return fail('operation-reused', 409);
                return json(200, { operation: c.operation, digest });
            }
            // Bind the operation ID before reserving capacity or storing content. Two
            // simultaneous requests cannot use the same permits for different envelopes.
            const intentKey = `${NS}c/${grant.collection}/operations/${c.operation}`;
            await first(kv, intentKey, b64json({ digest }));
            const prefix = `${updatesPrefix(grant.collection, c.document)}${grant.member}/${c.stream}/`;
            const newest = await kv.list({ prefix, limit: 1, valuesIncluded: false });
            const suffix = `${reverseSequence(c.sequence)}.${c.operation}`;
            const key = prefix + suffix;
            if (newest.entries[0] && newest.entries[0].key < key) return fail('sequence-superseded', 409);
            const slots = await reserve(kv, grant.collection, c.operation, bytes + 16384, key);
            if (await kv.get(deletedKey(grant.collection, c.document))) return fail('document-deleted', 410);
            // KV already transports binary as base64. Do not base64-wrap the large
            // ciphertext a second time inside JSON. Publish its small signed header
            // only after the immutable ciphertext survives a read-back check.
            const payload = payloadKey(key), expires = clock() + LIMITS.inactiveSeconds;
            await first(kv, payload, takeCiphertext(c), LIMITS.inactiveSeconds, runtime.crypto);
            const stored: Stored = { envelope: { ...envelope, checkpoint: { ...c, ciphertext: '' } }, author: member.signed, slots, payload };
            await first(kv, key, b64json(stored), Math.max(1, expires - clock()));
            // Recheck membership after storage. A concurrently revoked sender keeps its local outbox.
            if (await kv.get(deletedKey(grant.collection, c.document))) { await kv.delete(payload); await kv.delete(key); return fail('document-deleted', 410); }
            if (await kv.get(grantKey(grant.collection, grant.member)) !== b64json(member) || await kv.get(revokedKey(grant.collection, grant.member))) return fail('access-ended', 403);
            await first(kv, receiptKey, b64json({ digest }));
            await kv.delete(intentKey);
            // Keep the newest two checkpoints for this writer/stream. Each full body checkpoint
            // contains all previous local operations. A crash before pruning only wastes capacity.
            const older = await kv.list({ prefix, after: key, limit: 3, valuesIncluded: false });
            for (const entry of older.entries.slice(1)) {
                const old = await read<Stored>(kv, entry.key);
                // Large releases are split by authenticated maintenance, rather than
                // overrunning the invocation budget after a successful durable append.
                if (old && old.slots.length <= 8 && await kv.delete(entry.key)) {
                    if (old.payload) await kv.delete(old.payload);
                    await release(kv, old.slots, old.envelope.checkpoint.operation);
                }
            }
            return json(200, { operation: c.operation, digest });
        }
        if (action === 'touch-folder') {
            if (grant.document !== null || !permitted(grant, scope(request.document), 'tree') || collection.root?.kind !== 'folder') return fail('forbidden-folder-renewal', 403);
            const id = scope(request.document), folder = await read<{ id: string; kind: string; slot: string }>(kv, docKey(grant.collection, id));
            if (!folder || folder.kind !== 'folder' || await kv.get(deletedKey(grant.collection, id))) return fail('folder-expired', 410);
            const root = await kv.getEntry(docKey(grant.collection, collection.root.document));
            if (await kv.get(deletedKey(grant.collection, collection.root.document))) return fail('connection-ended', 410);
            if (!root.value || !root.expiresAt || root.expiresAt < clock() + LIMITS.inactiveSeconds - 3600) return fail('renew-folder-root-first', 409);
            const ttlSec = root.expiresAt - clock();
            await kv.set(folder.slot, b64json(id), { ttlSec }); await kv.set(docKey(grant.collection, id), b64json(folder), { ttlSec });
            return json(200, { expires: root.expiresAt });
        }
        if (action === 'touch-record') {
            // Renewal is paged by individual immutable records. A large collection
            // must not exceed the runtime's 200-operation budget in one request.
            const id = scope(request.document), prefix = updatesPrefix(grant.collection, id);
            if (typeof request.key !== 'string' || !request.key.startsWith(prefix) || request.key.length > 256) return fail('forbidden-checkpoint', 403);
            const raw = await kv.get(request.key);
            if (!raw) return fail('checkpoint-expired', 410);
            const stored = parse<Stored>(raw);
            for (const slot of stored.slots) {
                const prefix = slot.slice(0, slot.lastIndexOf('/') + 1), current = (await kv.list({ prefix, limit: 1, valuesIncluded: true })).entries[0];
                const permit = current?.value ? parse<Permit>(current.value) : null;
                if (current?.key !== slot || !permit?.busy || permit.operation !== stored.envelope.checkpoint.operation) return fail('capacity-permit-missing', 409);
            }
            const expires = clock() + LIMITS.inactiveSeconds;
            if (stored.payload) {
                const ciphertext = await kv.get(stored.payload);
                if (!ciphertext) return fail('checkpoint-expired', 410);
                await kv.set(stored.payload, ciphertext, { ttlSec: LIMITS.inactiveSeconds });
            }
            await kv.set(request.key, raw, { ttlSec: Math.max(1, expires - clock()) });
            return json(200, { expires: (await kv.getEntry(request.key)).expiresAt });
        }
        if (action === 'touch') {
            const id = scope(request.document), document = await read<{ id: string; kind: string; slot: string }>(kv, docKey(grant.collection, id));
            if (!document) return fail('document-expired', 410);
            const entries = await allKeys(kv, updatesPrefix(grant.collection, id), 512);
            const heads = new Map<string, string>();
            for (const { key } of entries) {
                const stream = key.slice(updatesPrefix(grant.collection, id).length).split('/').slice(0, 2).join('/');
                if (!heads.has(stream)) heads.set(stream, key);
            }
            if (!heads.size || heads.size > 128) return fail('renewal-needs-checkpoint', 409);
            let expires = clock() + LIMITS.inactiveSeconds;
            for (const key of heads.values()) {
                const entry = await kv.getEntry(key);
                if (!entry.value || !entry.expiresAt || entry.expiresAt < clock() + LIMITS.inactiveSeconds - 3600) return fail('renewal-incomplete', 409);
                expires = Math.min(expires, entry.expiresAt);
            }
            // The activity lease never outlives the earliest required writer checkpoint.
            const ttlSec = expires - clock();
            await kv.set(document.slot, b64json(id), { ttlSec });
            await kv.set(docKey(grant.collection, id), b64json(document), { ttlSec });
            await kv.set(collectionKey(grant.collection), b64json(collection), { ttlSec: LIMITS.inactiveSeconds });
            // The key epoch must live as long as the document. Without its marker the epoch reads as 1
            // again, and every append at the real epoch fails with `key-epoch-changed`.
            const epochMarker = (await kv.list({ prefix: epochsPrefix(grant.collection, id), limit: 1, valuesIncluded: true })).entries[0];
            if (epochMarker?.value) await kv.set(epochMarker.key, epochMarker.value, { ttlSec: LIMITS.inactiveSeconds });
            if (!grant.expires) {
                // A device that stays offline for longer than the lease still needs its wrapped key for
                // the current epoch. Renew those keys while the request has operations to spare.
                const renewKeys = heads.size <= KEY_RENEWAL_HEAD_LIMIT;
                const peers = await kv.list({ prefix: `${NS}c/${grant.collection}/member-slots/`, limit: LIMITS.members, valuesIncluded: true });
                for (const entry of peers.entries) if (entry.value) {
                    const peerId = parse<string>(entry.value), peerKey = grantKey(grant.collection, peerId), peer = await kv.get(peerKey);
                    if (!peer || await kv.get(revokedKey(grant.collection, peerId))) continue;
                    await kv.set(peerKey, peer, { ttlSec: LIMITS.inactiveSeconds });
                    await kv.set(entry.key, entry.value, { ttlSec: LIMITS.inactiveSeconds });
                    if (!renewKeys) continue;
                    const wrapped = (await kv.list({ prefix: `${NS}c/${grant.collection}/keys/${peerId}/${id}/`, limit: 1, valuesIncluded: true })).entries[0];
                    if (wrapped?.value) await kv.set(wrapped.key, wrapped.value, { ttlSec: LIMITS.inactiveSeconds });
                }
            }
            return json(200, { expires });
        }
        return json(404, { error: 'unknown-sync-action' });
    } catch (error) {
        // Never expose runtime errors, request bodies, credentials, or cryptographic material.
        if (error instanceof SyncError) return json(error.status, { error: error.code });
        if (error instanceof SyntaxError) return json(400, { error: 'invalid-json' });
        return json(503, { error: 'sync-service-unavailable' });
    }
}

export const responderInternals = { signatureValid, reserve, release, allocate, first };
