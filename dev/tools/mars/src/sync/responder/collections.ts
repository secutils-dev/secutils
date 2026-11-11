// Collection creation, the one action that needs neither a grant nor an invitation. The owner
// signs its own grant, and a limit for each client address bounds the number of collections.
import { encode, fail, LIMITS, validGrant, validId } from '../protocol.ts';
import type { Context, Runtime, SyncRequest, Collection } from './types.ts';
import { signatureValid, allocate, NS, clock, first, collectionKey, b64json, grantKey, json } from './store.ts';

/**
 * Creates a collection from a self-signed owner grant and stores the owner as its first member.
 * Each client address can create a limited number of collections in one hour.
 */
export async function createCollection(context: Context, runtime: Runtime, request: SyncRequest) {
    const { kv } = runtime;

    const signed = request.owner;
    if (
        !validGrant(signed) ||
        signed.grant.permission !== 'owner' ||
        signed.grant.document !== null ||
        signed.grant.expires !== 0 ||
        !signatureValid(signed.grant, signed.signature, signed.grant.publicKey)
    )
        return fail('invalid-owner');
    const g = signed.grant,
        ip = String(context.clientAddress ?? 'unknown').replace(/:\d+$/, '');
    const ipHash = await runtime.crypto.sha256(encode(new TextEncoder().encode(ip)));
    await allocate(
        kv,
        `${NS}create/${ipHash}/${Math.floor(clock() / 3600)}/`,
        g.collection,
        LIMITS.collections,
        3600,
        'collection-creation-limit',
        429,
    );
    if (signed.issuer) return fail('invalid-owner');
    const root = request.root;
    if (root && (!validId(root.document) || !['doc', 'folder'].includes(root.kind))) return fail('invalid-root');
    const collection: Collection = {
        v: 1,
        owner: signed,
        ...(root ? { root: { document: root.document, kind: root.kind } } : {}),
    };
    await first(kv, collectionKey(g.collection), b64json(collection));
    const slot = await allocate(kv, `${NS}c/${g.collection}/member-slots/`, g.member, LIMITS.members);
    await first(kv, grantKey(g.collection, g.member), b64json({ signed, slot }));
    return json(201, { collection: g.collection, limits: LIMITS });
}
