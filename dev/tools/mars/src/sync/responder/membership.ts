// Member actions for access: the grants of devices and Live recipients, the invitations behind
// Sync and Live links, the owner side of pairing, and revocation. Except for `bootstrap`, each
// action needs the owner grant, and each stored grant or invitation carries an owner signature.
import {
    canonical,
    fail,
    LIMITS,
    validGrant,
    validId,
    validInvitation,
    validBase64,
    type SignedGrant,
} from '../protocol.ts';
import type { Member, InviteRecord } from './types.ts';
import {
    json,
    liveKeyUpdate,
    NS,
    parse,
    signatureValid,
    revokedInviteKey,
    clock,
    read,
    docKey,
    deletedKey,
    inviteKey,
    allocate,
    first,
    b64json,
    signedByOwner,
    revokedKey,
    grantKey,
} from './store.ts';
import type { MemberSession } from './session.ts';

/**
 * Returns the grant of the caller, its wrapped keys, the owner grant, and the root, for a device that starts or
 * resumes.
 */
export async function bootstrap(session: MemberSession) {
    const { kv, collection, member, grant } = session;

    return json(200, {
        signed: member.signed,
        keys: member.keys ?? [],
        owner: collection.owner,
        root: grant.document ? { document: grant.document, kind: 'doc' } : collection.root,
        keyUpdate: member.signed.issuer
            ? await liveKeyUpdate(kv, member.signed.issuer.invitation.id, grant.collection, grant.document!)
            : null,
    });
}
/**
 * Lists every member grant of the collection. Owner only.
 */
export async function listMembers(session: MemberSession) {
    const { kv, grant, owner } = session;

    owner();
    const entries = await kv.list({
        prefix: `${NS}c/${grant.collection}/members/`,
        limit: LIMITS.members + LIMITS.liveGrants,
        valuesIncluded: true,
    });
    return json(200, {
        members: entries.entries.flatMap((entry) => (entry.value ? [parse<Member>(entry.value).signed] : [])),
    });
}
/**
 * Creates a Sync or Live invitation from a policy that the owner signed. The record holds the token hash and, for a
 * short link, the encrypted bootstrap. Owner only.
 */
export async function createInvitation(session: MemberSession) {
    const { kv, request, collection, grant, owner } = session;

    owner();
    const policy = request.policy;
    if (
        !validInvitation(policy) ||
        policy.invitation.collection !== grant.collection ||
        !signatureValid(policy.invitation, policy.signature, collection.owner.grant.publicKey)
    )
        return fail('invalid-invitation');
    const p = policy.invitation;
    if (await kv.get(revokedInviteKey(grant.collection, p.id))) return fail('invitation-revoked', 403);
    if (
        p.expires <= clock() ||
        p.expires > clock() + (p.mode === 'pair' ? LIMITS.invitationSeconds : LIMITS.maxLiveSeconds) ||
        (p.mode === 'pair' && p.participant !== collection.owner.grant.participant)
    )
        return fail('invalid-invitation-lifetime');
    if (p.document) {
        const doc = await read<{ kind: string }>(kv, docKey(grant.collection, p.document));
        if (!doc || doc.kind !== 'doc' || (await kv.get(deletedKey(grant.collection, p.document))))
            return fail('invalid-live-document');
    }
    if (typeof request.tokenHash !== 'string' || !/^[a-f0-9]{64}$/.test(request.tokenHash))
        return fail('invalid-invitation-token');
    const bootstrap = request.bootstrap;
    if (
        bootstrap !== undefined &&
        (!bootstrap ||
            typeof bootstrap !== 'object' ||
            Object.keys(bootstrap).sort().join(',') !== 'ciphertext,nonce' ||
            !validBase64(bootstrap.nonce, 16) ||
            bootstrap.nonce.length !== 16 ||
            !validBase64(bootstrap.ciphertext, 16384) ||
            bootstrap.ciphertext.length < 22)
    )
        return fail('invalid-live-bootstrap');
    // Invitation ids share one namespace. Refuse an id that holds a different invitation before
    // a slot points at it: cleanup of this collection follows its slots.
    const existing = await read<InviteRecord>(kv, inviteKey(p.id));
    if (existing && canonical(existing.policy) !== canonical(policy)) return fail('immutable-record-conflict', 409);
    await allocate(
        kv,
        `${NS}c/${grant.collection}/invitation-slots/`,
        p.id,
        LIMITS.liveGrants,
        Math.max(1, p.expires - clock()),
    );
    await first(
        kv,
        inviteKey(p.id),
        b64json({
            policy,
            tokenHash: request.tokenHash,
            inviter: grant.member,
            ...(bootstrap ? { bootstrap } : {}),
        }),
        Math.max(1, p.expires - clock()),
    );
    return json(201, { invitation: p.id });
}
/**
 * Serves the owner side of pairing: `pair-pending` returns the request of the joining device, and `pair-approve`
 * stores the grant that the owner signed for it. Owner only.
 */
export async function pairApproval(session: MemberSession) {
    const { kv, action, request, collection, grant, owner, wrappedKeys } = session;

    owner();
    if (!validId(request.invitation)) return fail('invalid-invitation');
    const entry = await read<InviteRecord>(kv, inviteKey(request.invitation));
    if (
        !entry ||
        entry.policy.invitation.collection !== grant.collection ||
        entry.policy.invitation.mode !== 'pair' ||
        entry.policy.invitation.expires <= clock()
    )
        return fail('invitation-expired', 410);
    const pending = await read<{ candidate: SignedGrant; label: unknown }>(
        kv,
        `${inviteKey(request.invitation)}/request`,
    );
    if (action === 'pair-pending') return json(200, pending ?? { candidate: null });
    const signed = request.signed;
    if (
        !pending ||
        !validGrant(signed) ||
        signed.issuer ||
        canonical(signed.grant) !== canonical(pending.candidate.grant) ||
        !signedByOwner(signed, collection.owner)
    )
        return fail('invalid-pair-approval', 403);
    const keys = wrappedKeys(signed, request.keys),
        g = signed.grant;
    if (await kv.get(revokedKey(grant.collection, g.member))) return fail('member-revoked', 403);
    const slot = await allocate(kv, `${NS}c/${grant.collection}/member-slots/`, g.member, LIMITS.members);
    await first(kv, grantKey(grant.collection, g.member), b64json({ signed, slot, keys }));
    await first(
        kv,
        `${inviteKey(request.invitation)}/approved`,
        b64json({ member: g.member }),
        Math.max(1, entry.policy.invitation.expires - clock()),
    );
    return json(200, { approved: true });
}
/**
 * Ends an invitation: writes a persistent revocation fence, then deletes the record. A repeated request succeeds.
 * Owner only.
 */
export async function revokeInvitation(session: MemberSession) {
    const { kv, request, grant, owner } = session;

    owner();
    if (!validId(request.invitation)) return fail('invalid-invitation');
    if (await kv.get(revokedInviteKey(grant.collection, request.invitation))) return json(200, { revoked: true });
    const entry = await read<InviteRecord>(kv, inviteKey(request.invitation));
    if (!entry || entry.policy.invitation.collection !== grant.collection) return fail('forbidden-invitation', 403);
    await first(kv, revokedInviteKey(grant.collection, request.invitation), b64json({ revoked: true }), null);
    await kv.delete(inviteKey(request.invitation));
    return json(200, { revoked: true });
}
/**
 * Stores a member grant that the owner signed: a device grant without expiry, or a document grant with a bounded
 * lifetime. Owner only.
 */
export async function grantAccess(session: MemberSession) {
    const { kv, request, collection, grant, scope, owner, wrappedKeys } = session;

    owner();
    const signed = request.signed;
    if (
        !validGrant(signed) ||
        signed.grant.collection !== grant.collection ||
        signed.grant.permission === 'owner' ||
        !signatureValid(signed.grant, signed.signature, collection.owner.grant.publicKey)
    )
        return fail('invalid-grant');
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
/**
 * Revokes a member: writes a persistent fence, deletes the grant, and frees its slot. An unknown member is a success
 * without a fence. Owner only.
 */
export async function revokeMember(session: MemberSession) {
    const { kv, request, collection, grant, owner } = session;

    owner();
    if (!validId(request.target) || request.target === collection.owner.grant.member) return fail('invalid-revocation');
    // An immutable fence prevents a late activity request from restoring access.
    // Keep this small, opaque identity fence until maintenance proves all grants expired.
    // The fence never expires. Write it only for a member that has, or had, a grant here, so
    // requests with random ids cannot add permanent records.
    const target = await read<Member>(kv, grantKey(grant.collection, request.target));
    if (!target && !(await kv.get(revokedKey(grant.collection, request.target))))
        return json(200, { revoked: request.target });
    await kv.set(revokedKey(grant.collection, request.target), b64json({ at: clock() }), { ifAbsent: true });
    if ((await kv.getEntry(revokedKey(grant.collection, request.target))).expiresAt !== null)
        return fail('persistent-revocation-unavailable', 503);
    await kv.delete(grantKey(grant.collection, request.target));
    // Free the slot. A revoked Live member otherwise holds one of the collection's slots until its grant expires.
    if (target && (await kv.get(target.slot)) === b64json(request.target)) await kv.delete(target.slot);
    return json(200, { revoked: request.target });
}
