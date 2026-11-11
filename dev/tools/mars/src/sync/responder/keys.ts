// Member actions for document keys. The service stores keys only in wrapped or encrypted form:
// one wrapped key for each member, document, and epoch, and one encrypted key update for each
// Live invitation and epoch. The epoch marker of a document names the key that writers must use.
import { fail, reverseSequence, validId, validWrappedKey, validBase64 } from '../protocol.ts';
import type { InviteRecord } from './types.ts';
import {
    MAX_KEY_WRITES,
    exactKeys,
    signatureValid,
    grantKey,
    NS,
    first,
    b64json,
    deletedKey,
    json,
    parse,
    epochsPrefix,
    read,
    inviteKey,
    validSealed,
    MAX_LIVE_KEY_CHARS,
    currentEpoch,
    clock,
} from './store.ts';
import type { MemberSession } from './session.ts';

/**
 * Stores wrapped document keys that the owner signed, one for each member, document, and epoch, and prunes all but the
 * newest two epochs. Owner only.
 */
export async function writeKeys(session: MemberSession) {
    const { kv, request, collection, grant, owner } = session;

    owner();
    const values = request.values;
    if (!Array.isArray(values) || values.length > MAX_KEY_WRITES) return fail('invalid-key-update');
    const written: string[] = [],
        members = new Map<string, boolean>();
    for (const value of values) {
        // Exact shape: an extra field would store unmetered bytes beside the small envelope.
        if (
            !exactKeys(value, ['signature', 'wrapped']) ||
            !validBase64(value.signature, 86) ||
            !validWrappedKey(value.wrapped) ||
            value.wrapped.collection !== grant.collection ||
            value.wrapped.sender !== collection.owner.grant.boxKey ||
            !signatureValid(value.wrapped, value.signature, collection.owner.grant.publicKey)
        )
            return fail('invalid-key-signature', 403);
        // A key for a member without a grant has no reader. Skip it, so key records stay
        // bounded by the membership. An expired member in a saved rotation plan is not an error.
        const target = value.wrapped.member;
        if (!members.has(target)) members.set(target, !!(await kv.get(grantKey(grant.collection, target))));
        if (!members.get(target)) continue;
        const key = `${NS}c/${grant.collection}/keys/${value.wrapped.member}/${value.wrapped.document}/${reverseSequence(value.wrapped.epoch)}`;
        await first(kv, key, b64json(value));
        written.push(key);
        const older = await kv.list({
            prefix: key.slice(0, key.lastIndexOf('/') + 1),
            after: key,
            limit: 3,
            valuesIncluded: false,
        });
        for (const entry of older.entries.slice(1)) await kv.delete(entry.key);
    }
    if (collection.root?.kind === 'doc' && (await kv.get(deletedKey(grant.collection, collection.root.document)))) {
        for (const key of written) await kv.delete(key);
        return fail('connection-ended', 410);
    }
    return json(200, { written: values.length });
}
/**
 * Returns the newest two wrapped keys of the caller for one document.
 */
export async function readKeys(session: MemberSession) {
    const { kv, request, grant, scope } = session;

    const id = scope(request.document),
        prefix = `${NS}c/${grant.collection}/keys/${grant.member}/${id}/`;
    const page = await kv.list({ prefix, limit: 2, valuesIncluded: true });
    return json(200, { values: page.entries.flatMap((entry) => (entry.value ? [parse(entry.value)] : [])) });
}
/**
 * Returns the current key epoch of one document.
 */
export async function readEpoch(session: MemberSession) {
    const { kv, request, grant, scope } = session;

    const id = scope(request.document),
        page = await kv.list({ prefix: epochsPrefix(grant.collection, id), limit: 1, valuesIncluded: true });
    return json(200, {
        epoch: page.entries[0]?.value ? parse<{ epoch: number }>(page.entries[0].value).epoch : 1,
    });
}
/**
 * Activates the next key epoch of a document. The epoch moves by exactly one, and a repeated request for the current
 * epoch succeeds. Owner only.
 */
export async function rotateDocument(session: MemberSession) {
    const { kv, request, grant, scope, owner } = session;

    owner();
    const id = scope(request.document),
        epoch = request.epoch;
    const page = await kv.list({ prefix: epochsPrefix(grant.collection, id), limit: 1, valuesIncluded: true });
    const current = page.entries[0]?.value ? parse<{ epoch: number }>(page.entries[0].value).epoch : 1;
    if (!Number.isSafeInteger(epoch) || (epoch !== current && epoch !== current + 1))
        return fail('invalid-key-epoch', 409);
    if (epoch === current) return json(200, { epoch });
    await first(kv, epochsPrefix(grant.collection, id) + reverseSequence(epoch), b64json({ epoch }));
    return json(200, { epoch });
}
/**
 * Stores the owner-signed, encrypted key update of a Live invitation for the current or the next epoch. Owner only.
 */
export async function writeLiveKey(session: MemberSession) {
    const { kv, request, collection, grant, owner } = session;

    owner();
    if (!validId(request.invitation)) return fail('invalid-invitation');
    const invite = await read<InviteRecord>(kv, inviteKey(request.invitation));
    if (!invite || invite.policy.invitation.collection !== grant.collection || invite.policy.invitation.mode !== 'live')
        return fail('forbidden-invitation', 403);
    const value = request.value;
    if (
        !exactKeys(value, ['ciphertext', 'document', 'epoch', 'nonce']) ||
        !validSealed({ nonce: value.nonce, ciphertext: value.ciphertext }, MAX_LIVE_KEY_CHARS) ||
        !validBase64(request.signature, 86) ||
        value.document !== invite.policy.invitation.document ||
        !signatureValid(value, request.signature, collection.owner.grant.publicKey)
    )
        return fail('invalid-key-signature', 403);
    // A key update exists for the current epoch or for the next one, which a rotation activates afterwards.
    if (
        !Number.isSafeInteger(value.epoch) ||
        value.epoch < 1 ||
        value.epoch > (await currentEpoch(kv, grant.collection, value.document)) + 1
    )
        return fail('invalid-key-epoch');
    const key = `${inviteKey(request.invitation)}/keys/${reverseSequence(value.epoch)}`;
    await first(
        kv,
        key,
        b64json({ value, signature: request.signature }),
        Math.max(1, invite.policy.invitation.expires - clock()),
    );
    if (await kv.get(deletedKey(grant.collection, value.document))) {
        await kv.delete(key);
        return fail('document-deleted', 410);
    }
    return json(200, { updated: true });
}
