// Member actions that extend leases. Content expires after `LIMITS.inactiveSeconds` without
// activity. A client renews the checkpoints first and then the document, so the document lease
// never outlives a checkpoint that readers need.
import { fail, LIMITS, permitted } from '../protocol.ts';
import type { Stored, Permit } from './types.ts';
import {
    read,
    docKey,
    deletedKey,
    clock,
    b64json,
    json,
    updatesPrefix,
    parse,
    allKeys,
    collectionKey,
    epochsPrefix,
    KEY_RENEWAL_HEAD_LIMIT,
    NS,
    grantKey,
    revokedKey,
} from './store.ts';
import type { MemberSession } from './session.ts';

/**
 * Extends the lease of a nested folder to the lease of the collection root. The root must be renewed first.
 */
export async function renewFolder(session: MemberSession) {
    const { kv, request, collection, grant, scope } = session;

    if (
        grant.document !== null ||
        !permitted(grant, scope(request.document), 'tree') ||
        collection.root?.kind !== 'folder'
    )
        return fail('forbidden-folder-renewal', 403);
    const id = scope(request.document),
        folder = await read<{ id: string; kind: string; slot: string }>(kv, docKey(grant.collection, id));
    if (!folder || folder.kind !== 'folder' || (await kv.get(deletedKey(grant.collection, id))))
        return fail('folder-expired', 410);
    const root = await kv.getEntry(docKey(grant.collection, collection.root.document));
    if (await kv.get(deletedKey(grant.collection, collection.root.document))) return fail('connection-ended', 410);
    if (!root.value || !root.expiresAt || root.expiresAt < clock() + LIMITS.inactiveSeconds - 3600)
        return fail('renew-folder-root-first', 409);
    const ttlSec = root.expiresAt - clock();
    await kv.set(folder.slot, b64json(id), { ttlSec });
    await kv.set(docKey(grant.collection, id), b64json(folder), { ttlSec });
    return json(200, { expires: root.expiresAt });
}
/**
 * Extends the lease of one checkpoint and its ciphertext, when its capacity permits are still held.
 */
export async function renewCheckpoint(session: MemberSession) {
    const { kv, request, grant, scope } = session;

    // Renewal is paged by individual immutable records. A large collection
    // must not exceed the runtime's 200-operation budget in one request.
    const id = scope(request.document),
        prefix = updatesPrefix(grant.collection, id);
    if (typeof request.key !== 'string' || !request.key.startsWith(prefix) || request.key.length > 256)
        return fail('forbidden-checkpoint', 403);
    const raw = await kv.get(request.key);
    if (!raw) return fail('checkpoint-expired', 410);
    const stored = parse<Stored>(raw);
    for (const slot of stored.slots) {
        const prefix = slot.slice(0, slot.lastIndexOf('/') + 1),
            current = (await kv.list({ prefix, limit: 1, valuesIncluded: true })).entries[0];
        const permit = current?.value ? parse<Permit>(current.value) : null;
        if (current?.key !== slot || !permit?.busy || permit.operation !== stored.envelope.checkpoint.operation)
            return fail('capacity-permit-missing', 409);
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
/**
 * Extends the lease of a document after its newest checkpoints were renewed, together with the collection record, the
 * key epoch, and the device grants and keys.
 */
export async function renewDocument(session: MemberSession) {
    const { kv, request, collection, grant, scope } = session;

    const id = scope(request.document),
        document = await read<{ id: string; kind: string; slot: string }>(kv, docKey(grant.collection, id));
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
        if (!entry.value || !entry.expiresAt || entry.expiresAt < clock() + LIMITS.inactiveSeconds - 3600)
            return fail('renewal-incomplete', 409);
        expires = Math.min(expires, entry.expiresAt);
    }
    // The activity lease never outlives the earliest required writer checkpoint.
    const ttlSec = expires - clock();
    await kv.set(document.slot, b64json(id), { ttlSec });
    await kv.set(docKey(grant.collection, id), b64json(document), { ttlSec });
    await kv.set(collectionKey(grant.collection), b64json(collection), { ttlSec: LIMITS.inactiveSeconds });
    // The key epoch must live as long as the document. Without its marker the epoch reads as 1
    // again, and every append at the real epoch fails with `key-epoch-changed`.
    const epochMarker = (await kv.list({ prefix: epochsPrefix(grant.collection, id), limit: 1, valuesIncluded: true }))
        .entries[0];
    if (epochMarker?.value) await kv.set(epochMarker.key, epochMarker.value, { ttlSec: LIMITS.inactiveSeconds });
    if (!grant.expires) {
        // A device that stays offline for longer than the lease still needs its wrapped key for
        // the current epoch. Renew those keys while the request has operations to spare.
        const renewKeys = heads.size <= KEY_RENEWAL_HEAD_LIMIT;
        const peers = await kv.list({
            prefix: `${NS}c/${grant.collection}/member-slots/`,
            limit: LIMITS.members,
            valuesIncluded: true,
        });
        for (const entry of peers.entries)
            if (entry.value) {
                const peerId = parse<string>(entry.value),
                    peerKey = grantKey(grant.collection, peerId),
                    peer = await kv.get(peerKey);
                if (!peer || (await kv.get(revokedKey(grant.collection, peerId)))) continue;
                await kv.set(peerKey, peer, { ttlSec: LIMITS.inactiveSeconds });
                await kv.set(entry.key, entry.value, { ttlSec: LIMITS.inactiveSeconds });
                if (!renewKeys) continue;
                const wrapped = (
                    await kv.list({
                        prefix: `${NS}c/${grant.collection}/keys/${peerId}/${id}/`,
                        limit: 1,
                        valuesIncluded: true,
                    })
                ).entries[0];
                if (wrapped?.value) await kv.set(wrapped.key, wrapped.value, { ttlSec: LIMITS.inactiveSeconds });
            }
    }
    return json(200, { expires });
}
