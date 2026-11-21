// Exact-collection operator retirement. Retain the fence and never delete a global slot head:
// deleting the head can resurrect a reservation or let an old retry reuse a generation.
import { encode, fail, reverseSequence, validHash, validId } from '../protocol.ts';
import { b64json, clock, collectionKey, inviteKey, NS, parse, read, retirementKey } from './store.ts';
import type { Collection, InviteRecord, Permit, Runtime, SyncRequest } from './types.ts';

const BATCH = 32;
const DRAIN_SECONDS = 120;
const GLOBAL = `${NS}budget/all/`;
const INVITATIONS = `${NS}invitations/`;

/** Resolve an audit fingerprint without returning raw identifiers of any nonmatching collection. */
async function resolveCollection(runtime: Runtime, fingerprint: string, retired: boolean, after?: string) {
    const prefix = retired ? retirementKey('') : `${NS}c/`;
    for (let n = 0; n < BATCH; n++) {
        const result = await runtime.kv.list({ prefix, after, limit: 1, valuesIncluded: false });
        if (!result.entries.length) return { complete: true };
        const id = result.entries[0].key.split('/')[retired ? 2 : 3];
        if (!validId(id)) return fail('invalid-collection-key', 409);
        if ((await fingerprintOf(runtime, id)) === fingerprint) return { complete: true, collection: id };
        after = retired ? `${prefix}${id}` : `${prefix}${id}0`; // '/' sorts before '0', after every key of this id.
    }
    return { complete: false, cursor: after };
}

const fingerprintOf = (runtime: Runtime, id: string) =>
    runtime.crypto.sha256(encode(new TextEncoder().encode(`mars-storage-audit-v1:collection:${id}`)));

/** Plan, fence, and page one approved collection. Repeating a page does not touch other collections. */
export async function retireCollection(request: SyncRequest, runtime: Runtime) {
    const { kv } = runtime;
    const { action, fingerprint, collection, after } = request;
    if (!validHash(fingerprint)) return fail('invalid-collection-fingerprint');
    const cursor = (prefix: string) => {
        if (after == null) return undefined;
        if (typeof after !== 'string' || after.length > 512 || !after.startsWith(prefix))
            return fail('invalid-retirement-cursor');
        return after;
    };
    if (action === 'resolve' || action === 'resolve-retired') {
        const retired = action === 'resolve-retired';
        return resolveCollection(runtime, fingerprint, retired, cursor(retired ? retirementKey('') : `${NS}c/`));
    }
    if (!validId(collection) || (await fingerprintOf(runtime, collection)) !== fingerprint)
        return fail('collection-fingerprint-mismatch', 409);
    const fence = retirementKey(collection);
    const started = await read<{ started: number }>(kv, fence);
    const owner =
        action === 'plan' || (!started && action === 'begin')
            ? await read<Collection>(kv, collectionKey(collection))
            : null;
    if (action === 'plan') return { matched: owner?.owner.grant.collection === collection, fenced: !!started };
    if (action === 'begin') {
        if (!started) {
            if (owner?.owner.grant.collection !== collection) return fail('collection-not-found', 404);
            await kv.set(fence, b64json({ started: clock() }), { ifAbsent: true });
        }
        const entry = await kv.getEntry(fence);
        if (!entry.value || entry.expiresAt !== null) return fail('permanent-retirement-fence-unavailable', 503);
        return { notBefore: parse<{ started: number }>(entry.value).started + DRAIN_SECONDS };
    }
    if (!started || started.started + DRAIN_SECONDS > clock()) return fail('retirement-not-ready', 409);
    if (action === 'global') {
        const page = await kv.list({ prefix: GLOBAL, after: cursor(GLOBAL), limit: BATCH, valuesIncluded: true });
        let released = 0,
            removed = 0;
        for (const entry of page.entries) {
            if (!entry.value) continue;
            const permit = parse<Permit>(entry.value);
            if (permit.collection !== collection) continue;
            const slot = entry.key.slice(0, entry.key.lastIndexOf('/') + 1);
            let head = (await kv.list({ prefix: slot, limit: 1, valuesIncluded: false })).entries[0];
            if (head?.key === entry.key && permit.busy) {
                const free = `${slot}${reverseSequence(permit.generation + 1)}`;
                await kv.set(free, b64json({ ...permit, busy: false, generation: permit.generation + 1 }), {
                    ifAbsent: true,
                });
                head = (await kv.list({ prefix: slot, limit: 1, valuesIncluded: false })).entries[0];
                if (head?.key < entry.key) released++;
            }
            if (head?.key < entry.key && (await kv.delete(entry.key))) removed++;
        }
        return { complete: page.entries.length < BATCH, cursor: page.cursor, released, removed };
    }
    if (action === 'invitations') {
        const page = await kv.list({
            prefix: INVITATIONS,
            after: cursor(INVITATIONS),
            limit: BATCH,
            valuesIncluded: false,
        });
        let removed = 0;
        for (const entry of page.entries) {
            const parts = entry.key.split('/');
            if (parts.length !== 4 || !validId(parts[3])) continue;
            const owned = await read<InviteRecord>(kv, entry.key);
            if (owned?.policy.invitation.collection !== collection) continue;
            const support = await kv.list({ prefix: `${inviteKey(parts[3])}/`, limit: BATCH, valuesIncluded: false });
            for (const row of support.entries) if (await kv.delete(row.key)) removed++;
            if (support.entries.length < BATCH && (await kv.delete(entry.key))) removed++;
            // Retry this page, including the main record if its support did not fit.
            return { complete: false, cursor: after ?? null, removed, retry: true };
        }
        return { complete: page.entries.length < BATCH, cursor: page.cursor, removed };
    }
    const prefixes: Record<string, string> = {
        records: `${NS}c/${collection}/`,
        budget: `${NS}budget/${collection}/`,
        refs: `${NS}reservations/${collection}/`,
    };
    if (!Object.hasOwn(prefixes, action)) return fail('invalid-retirement-action');
    const page = await kv.list({ prefix: prefixes[action], limit: BATCH, valuesIncluded: false });
    let removed = 0;
    for (const entry of page.entries) if (await kv.delete(entry.key)) removed++;
    return { complete: page.entries.length < BATCH, cursor: null, removed, retry: true };
}
