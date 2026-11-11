// Actions that the bearer token of an invitation authorizes, without a grant: a device that
// follows a Sync link, and a recipient that follows a Live link. The token hash, the expiry, and
// the revocation and deletion fences are checked before each action.
import { canonical, fail, LIMITS, validGrant, validId } from '../protocol.ts';
import type { Context, Runtime, SyncRequest, InviteRecord, Collection, Member } from './types.ts';
import {
    read,
    inviteKey,
    clock,
    collectionKey,
    revokedKey,
    revokedInviteKey,
    deletedKey,
    json,
    liveKeyUpdate,
    signedByOwner,
    allocate,
    NS,
    first,
    grantKey,
    b64json,
    signatureValid,
    validSealed,
    MAX_LABEL_CHARS,
} from './store.ts';

/**
 * Handles the actions that the bearer token of an invitation authorizes: `invite-read`, `pair-request`, `pair-result`,
 * and `live-join`.
 * It checks expiry, the token, and the revocation and deletion fences before any action.
 */
export async function invitationAction(context: Context, runtime: Runtime, request: SyncRequest, action: string) {
    const { kv } = runtime;

    if (!validId(request.invitation)) return fail('invalid-invitation');
    const entry = await read<InviteRecord>(kv, inviteKey(request.invitation));
    const secret = context.headers?.authorization?.match(/^Bearer ([A-Za-z0-9_-]{43})$/)?.[1];
    if (!entry || entry.policy.invitation.expires <= clock()) return fail('invitation-expired', 410);
    if (!secret || (await runtime.crypto.sha256(secret)) !== entry.tokenHash) return fail('forbidden-invitation', 403);
    const p = entry.policy.invitation,
        collection = await read<Collection>(kv, collectionKey(p.collection));
    if (
        !collection ||
        (await kv.get(revokedKey(p.collection, entry.inviter))) ||
        (await kv.get(revokedInviteKey(p.collection, p.id)))
    )
        return fail('connection-ended', 410);
    if (p.document && (await kv.get(deletedKey(p.collection, p.document)))) return fail('document-deleted', 410);
    if (!p.document && collection.root && (await kv.get(deletedKey(p.collection, collection.root.document))))
        return fail('connection-ended', 410);
    if (action === 'invite-read')
        return json(200, {
            policy: entry.policy,
            owner: collection.owner,
            ...(entry.bootstrap ? { bootstrap: entry.bootstrap } : {}),
            root: p.document ? { document: p.document, kind: 'doc' } : collection.root,
            keyUpdate: p.mode === 'live' ? await liveKeyUpdate(kv, p.id, p.collection, p.document!) : null,
        });
    if (action === 'live-join') {
        const signed = request.signed;
        if (
            p.mode !== 'live' ||
            !validGrant(signed) ||
            !signed.issuer ||
            canonical(signed.issuer) !== canonical(entry.policy) ||
            !signedByOwner(signed, collection.owner)
        )
            return fail('invalid-live-grant', 403);
        const g = signed.grant;
        if (g.expires <= clock() || (await kv.get(revokedKey(g.collection, g.member))))
            return fail('access-ended', 403);
        const slot = await allocate(
            kv,
            `${NS}c/${g.collection}/live-slots/`,
            g.member,
            LIMITS.liveGrants,
            Math.max(1, g.expires - clock()),
        );
        await first(kv, grantKey(g.collection, g.member), b64json({ signed, slot }), Math.max(1, g.expires - clock()));
        return json(201, { member: g.member });
    }
    if (p.mode !== 'pair') return fail('invalid-invitation-mode');
    const pendingKey = `${inviteKey(p.id)}/request`;
    if (action === 'pair-request') {
        const candidate = request.candidate;
        if (
            !validGrant(candidate) ||
            candidate.issuer ||
            candidate.grant.collection !== p.collection ||
            candidate.grant.permission !== 'edit' ||
            candidate.grant.document !== null ||
            candidate.grant.participant !== p.participant ||
            candidate.grant.expires !== 0 ||
            !signatureValid(candidate.grant, candidate.signature, candidate.grant.publicKey)
        )
            return fail('invalid-pair-request');
        // The label is the encrypted device name. Bound it: this endpoint needs no grant.
        const label = request.label ?? null;
        if (label !== null && !validSealed(label, MAX_LABEL_CHARS)) return fail('invalid-pair-request');
        await first(kv, pendingKey, b64json({ candidate, label }), Math.max(1, p.expires - clock()));
        return json(202, { requested: true });
    }
    const approved = await read<{ member: string }>(kv, `${inviteKey(p.id)}/approved`);
    if (!approved) return json(200, { approved: false });
    const member = await read<Member>(kv, grantKey(p.collection, approved.member));
    if (!member || (await kv.get(revokedKey(p.collection, approved.member)))) return fail('access-ended', 403);
    return json(200, { approved: true, ...member, owner: collection.owner, root: collection.root });
}
