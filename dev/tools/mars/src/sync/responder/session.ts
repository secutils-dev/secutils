// Authentication of a member request. Every member action runs with the `MemberSession` that
// `authenticate` returns, so the grant, revocation, expiry, and slot checks exist in one place.
import { canonical, fail, LIMITS, validId, validWrappedKey, type WrappedKey, type SignedGrant } from '../protocol.ts';
import type { Context, Runtime, SyncRequest, Collection, Member, InviteRecord } from './types.ts';
import {
    read,
    collectionKey,
    grantKey,
    allocate,
    NS,
    first,
    b64json,
    revokedKey,
    signedByOwner,
    clock,
    inviteKey,
    revokedInviteKey,
    parse,
    deletedKey,
} from './store.ts';

/**
 * Authenticates a member request and returns what every member action shares.
 *
 * The caller must present the bearer token of a grant that the collection owner signed. The grant must be
 * unrevoked and unexpired, must still hold its membership slot, and, for a Live member, must belong to an
 * invitation that still exists and is not revoked. The owner record restores itself after an expiry.
 *
 * `scope` checks a document id against the grant, `owner` requires the owner grant, `readDirectory` lists
 * the registered documents, and `wrappedKeys` validates the key envelopes of a new member.
 */
export async function authenticate(context: Context, runtime: Runtime, request: SyncRequest, action: string) {
    const { kv } = runtime;
    if (!validId(request.collection) || !validId(request.member)) return fail('invalid-scope');
    const collection = await read<Collection>(kv, collectionKey(request.collection));
    if (!collection) return fail('connection-expired', 410);
    let member = await read<Member>(kv, grantKey(request.collection, request.member));
    const token = context.headers?.authorization?.match(/^Bearer ([A-Za-z0-9_-]{43})$/)?.[1];
    if (
        !member &&
        token &&
        request.member === collection.owner.grant.member &&
        (await runtime.crypto.sha256(token)) === collection.owner.grant.tokenHash
    ) {
        const slot = await allocate(kv, `${NS}c/${request.collection}/member-slots/`, request.member, LIMITS.members);
        member = { signed: collection.owner, slot };
        await first(kv, grantKey(request.collection, request.member), b64json(member));
    }
    if (
        !member ||
        !token ||
        (await kv.get(revokedKey(request.collection, request.member))) ||
        (await runtime.crypto.sha256(token)) !== member.signed.grant.tokenHash ||
        !signedByOwner(member.signed, collection.owner) ||
        (await kv.get(member.slot)) !== b64json(request.member)
    )
        return fail('access-ended', 403);
    const grant = member.signed.grant;
    if (grant.expires && grant.expires <= clock()) return fail('access-expired', 403);
    if (member.signed.issuer) {
        // Invitation ids share one namespace across collections. Require the record of exactly
        // this invitation, and no revocation fence: another collection can create a record
        // with a revoked id, which must not restore the access of this member.
        const invitation = member.signed.issuer.invitation.id,
            invite = await read<InviteRecord>(kv, inviteKey(invitation));
        if (
            !invite ||
            canonical(invite.policy) !== canonical(member.signed.issuer) ||
            (await kv.get(revokedInviteKey(request.collection, invitation)))
        )
            return fail('live-link-ended', 403);
    }
    const scope = (id: unknown) => {
        if (!validId(id) || (grant.document !== null && grant.document !== id)) return fail('forbidden-document', 403);
        return id;
    };
    const owner = () => {
        if (grant.permission !== 'owner') return fail('owner-required', 403);
    };
    const readDirectory = async (deleted = new Map<string, boolean>()) => {
        if (grant.document !== null) return fail('forbidden-directory', 403);
        const page = await kv.list({
            prefix: `${NS}c/${grant.collection}/documents/`,
            limit: LIMITS.documents,
            valuesIncluded: true,
        });
        const documents = [];
        for (const entry of page.entries)
            if (entry.value) {
                const doc = parse<{ id: string; kind: string; creator?: string }>(entry.value);
                if (!deleted.has(doc.id)) deleted.set(doc.id, !!(await kv.get(deletedKey(grant.collection, doc.id))));
                documents.push({
                    id: doc.id,
                    kind: doc.kind,
                    creator: doc.creator ?? collection.owner.grant.member,
                    deleted: deleted.get(doc.id)!,
                });
            }
        return { documents, root: collection.root };
    };
    if (
        !['purge-live-document', 'deletions', 'delete-document'].includes(action) &&
        collection.root?.kind === 'doc' &&
        (await kv.get(deletedKey(grant.collection, collection.root.document)))
    )
        return fail('connection-ended', 410);
    const wrappedKeys = (signed: SignedGrant, values: unknown): WrappedKey[] => {
        if (values === undefined) return [];
        if (!Array.isArray(values) || values.length > LIMITS.documents || !values.every(validWrappedKey))
            return fail('invalid-key-envelopes');
        if (
            values.some(
                (key) =>
                    key.collection !== grant.collection ||
                    key.member !== signed.grant.member ||
                    key.sender !== collection.owner.grant.boxKey ||
                    (signed.grant.document !== null && key.document !== signed.grant.document),
            )
        )
            return fail('forbidden-key-envelope', 403);
        return values;
    };
    return {
        context,
        runtime,
        kv,
        action,
        request,
        collection,
        member,
        grant,
        scope,
        owner,
        readDirectory,
        wrappedKeys,
    };
}
/** The authenticated state of one member request. See `authenticate`. */
export type MemberSession = Awaited<ReturnType<typeof authenticate>>;
