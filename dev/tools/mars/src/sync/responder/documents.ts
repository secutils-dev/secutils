// Member actions for the documents of a collection: registration, the directory, deletion, and
// the stepwise purge of a Live-only collection. A deletion is a persistent fence, so a late
// request cannot restore the document.
import { fail, LIMITS, permitted } from '../protocol.ts';
import type { Member, InviteRecord, Stored } from './types.ts';
import {
    json,
    docKey,
    deletedKey,
    first,
    b64json,
    NS,
    parse,
    read,
    inviteKey,
    updatesPrefix,
    collectionKey,
    grantKey,
    allocate,
} from './store.ts';
import { release } from './quota.ts';
import type { MemberSession } from './session.ts';

/**
 * Returns the registered documents and folders of the collection, for a grant on the whole collection.
 */
export async function directory(session: MemberSession) {
    const { readDirectory } = session;

    return json(200, await readDirectory());
}
/**
 * Marks a registered document as deleted with a persistent fence. Reads, joins, and writes of that document fail
 * afterwards. Owner only.
 */
export async function deleteDocument(session: MemberSession) {
    const { kv, request, grant, scope, owner } = session;

    owner();
    const id = scope(request.document);
    // The fence never expires. Write it only for a registered document, so requests with
    // random ids cannot add permanent records.
    if ((await kv.get(docKey(grant.collection, id))) || (await kv.get(deletedKey(grant.collection, id))))
        await first(kv, deletedKey(grant.collection, id), b64json({ deleted: true }), null);
    return json(200, { deleted: id });
}
/**
 * Deletes a Live-only collection in bounded steps: fence first, then checkpoints, reservations, invitations, budget,
 * and the remaining records. Each request does one step, and the client repeats until `complete`. Owner only.
 */
export async function purgeLiveDocument(session: MemberSession) {
    const { kv, request, collection, member, grant, scope, owner } = session;

    owner();
    const id = scope(request.document);
    if (collection.root?.document !== id || collection.root.kind !== 'doc') return fail('personal-sync-active', 409);
    const started = !!(await kv.get(deletedKey(grant.collection, id)));
    const members = await kv.list({
        prefix: `${NS}c/${grant.collection}/members/`,
        limit: LIMITS.members + LIMITS.liveGrants,
        valuesIncluded: true,
    });
    if (
        !started &&
        members.entries.some(
            (entry) =>
                entry.value &&
                parse<Member>(entry.value).signed.grant.member !== grant.member &&
                parse<Member>(entry.value).signed.grant.document === null,
        )
    )
        return fail('personal-sync-active', 409);
    const documents = await kv.list({
        prefix: `${NS}c/${grant.collection}/documents/`,
        limit: LIMITS.documents,
        valuesIncluded: true,
    });
    if (!started && documents.entries.some((entry) => entry.value && parse<{ id: string }>(entry.value).id !== id))
        return fail('personal-sync-active', 409);
    const invitations = await kv.list({
        prefix: `${NS}c/${grant.collection}/invitation-slots/`,
        limit: LIMITS.liveGrants,
        valuesIncluded: true,
    });
    for (const slot of invitations.entries)
        if (slot.value) {
            const invitation = await read<InviteRecord>(kv, inviteKey(parse<string>(slot.value)));
            if (!started && invitation?.policy.invitation.mode === 'pair') return fail('personal-sync-active', 409);
        }
    // The irreversible fence blocks reads, joins, retries, and new writes.
    // Cleanup is bounded and repeatable, including after a lost response.
    await first(kv, deletedKey(grant.collection, id), b64json({ deleted: true }), null);
    const updates = await kv.list({
        prefix: updatesPrefix(grant.collection, id),
        limit: 1,
        valuesIncluded: true,
    });
    if (updates.entries[0]?.value) {
        const entry = updates.entries[0],
            stored = parse<Stored>(entry.value!);
        if (stored.payload) await kv.delete(stored.payload);
        const refs = await kv.list({
            prefix: `${NS}reservations/${grant.collection}/${stored.envelope.checkpoint.operation}/`,
            limit: 8,
            valuesIncluded: true,
        });
        await release(
            kv,
            refs.entries.flatMap((ref) => (ref.value ? [parse<string>(ref.value)] : [])),
            stored.envelope.checkpoint.operation,
            8,
            grant.collection,
        );
        if (
            !(
                await kv.list({
                    prefix: `${NS}reservations/${grant.collection}/${stored.envelope.checkpoint.operation}/`,
                    limit: 1,
                })
            ).entries.length
        )
            await kv.delete(entry.key);
        return json(200, { complete: false });
    }
    const refs = await kv.list({
        prefix: `${NS}reservations/${grant.collection}/`,
        limit: 8,
        valuesIncluded: true,
    });
    if (refs.entries.length) {
        const operations = new Set(refs.entries.map((ref) => ref.key.split('/')[4]));
        for (const operation of operations)
            await release(
                kv,
                refs.entries
                    .filter((ref) => ref.key.split('/')[4] === operation)
                    .flatMap((ref) => (ref.value ? [parse<string>(ref.value)] : [])),
                operation,
                8,
                grant.collection,
            );
        return json(200, { complete: false });
    }
    if (invitations.entries.length) {
        const slot = invitations.entries[0],
            invitation = slot.value ? parse<string>(slot.value) : null;
        // A slot can name an id that another collection holds. Delete only the records of an
        // invitation of this collection, or the leftovers of one whose main record is gone.
        const record = invitation ? await read<InviteRecord>(kv, inviteKey(invitation)) : null;
        const owned = !record || record.policy.invitation.collection === grant.collection;
        const entries =
            invitation && owned ? await kv.list({ prefix: inviteKey(invitation), limit: 16 }) : { entries: [] };
        for (const entry of entries.entries) await kv.delete(entry.key);
        if (entries.entries.length < 16) await kv.delete(slot.key);
        return json(200, { complete: false });
    }
    const budget = await kv.list({ prefix: `${NS}budget/${grant.collection}/`, limit: 16 });
    if (budget.entries.length) {
        for (const entry of budget.entries) await kv.delete(entry.key);
        return json(200, { complete: false });
    }
    const prefix = `${NS}c/${grant.collection}/`;
    if (
        request.after &&
        (typeof request.after !== 'string' || request.after.length > 256 || !request.after.startsWith(prefix))
    )
        return fail('invalid-cleanup-cursor');
    const page = await kv.list({ prefix, after: request.after || undefined, limit: 32 });
    const protectedKeys = new Set([
        collectionKey(grant.collection),
        grantKey(grant.collection, grant.member),
        member.slot,
    ]);
    for (const entry of page.entries)
        if (
            !protectedKeys.has(entry.key) &&
            !entry.key.includes('/revoked/') &&
            !entry.key.includes('/revoked-invitations/') &&
            !entry.key.includes('/deleted/')
        )
            await kv.delete(entry.key);
    if (page.entries.length === 32) return json(200, { complete: false, after: page.cursor });
    await kv.delete(member.slot);
    await kv.delete(grantKey(grant.collection, grant.member));
    await kv.delete(collectionKey(grant.collection));
    return json(200, { complete: true });
}
/**
 * Registers a document or folder in the collection and takes one of its document slots. Needs tree write access.
 */
export async function registerDocument(session: MemberSession) {
    const { kv, request, grant, scope } = session;

    const id = scope(request.document);
    if (await kv.get(deletedKey(grant.collection, id))) return fail('document-deleted', 410);
    if (!permitted(grant, id, 'tree') || !['doc', 'folder'].includes(request.kind))
        return fail('forbidden-document-create', 403);
    const slot = await allocate(kv, `${NS}c/${grant.collection}/document-slots/`, id, LIMITS.documents);
    await first(kv, docKey(grant.collection, id), b64json({ id, kind: request.kind, slot, creator: grant.member }));
    return json(201, { document: id });
}
/**
 * Returns the kind and the creator of a registered document.
 */
export async function documentInfo(session: MemberSession) {
    const { kv, request, collection, grant, scope } = session;

    const id = scope(request.document);
    if (await kv.get(deletedKey(grant.collection, id))) return fail('document-deleted', 410);
    const doc = await read<{ kind: string; creator?: string }>(kv, docKey(grant.collection, id));
    if (!doc) return fail('document-expired', 410);
    return json(200, { document: id, kind: doc.kind, creator: doc.creator ?? collection.owner.grant.member });
}
