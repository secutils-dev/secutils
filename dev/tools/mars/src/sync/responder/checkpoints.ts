// Member actions for checkpoints, the encrypted states that clients exchange: the index, one
// read, and the append. A checkpoint is immutable. Its small signed header and its ciphertext are
// separate records, and the header appears only after the ciphertext is stored.
import {
    checkpointReceipt,
    fail,
    jsonBytes,
    LIMITS,
    permitted,
    reverseSequence,
    validCheckpoint,
    validId,
} from '../protocol.ts';
import type { Stored } from './types.ts';
import {
    deletedKey,
    updatesPrefix,
    json,
    read,
    epochsPrefix,
    parse,
    signatureValid,
    docKey,
    b64json,
    NS,
    first,
    payloadKey,
    clock,
    takeCiphertext,
    grantKey,
    revokedKey,
} from './store.ts';
import { reserve, release } from './quota.ts';
import type { MemberSession } from './session.ts';

/**
 * Lists one page of checkpoint keys, newest first for each stream, without deleted or excluded documents. The first
 * page can include the directory.
 */
export async function listCheckpoints(session: MemberSession) {
    const { kv, request, collection, grant, scope, readDirectory } = session;

    if (collection.root && (await kv.get(deletedKey(grant.collection, collection.root.document))))
        return fail('connection-ended', 410);
    if (
        (grant.document || request.document) &&
        (await kv.get(deletedKey(grant.collection, scope(grant.document ?? request.document))))
    )
        return fail('document-deleted', 410);
    const prefix = updatesPrefix(grant.collection, grant.document ?? (request.document ? scope(request.document) : ''));
    if (
        request.after &&
        (typeof request.after !== 'string' || !request.after.startsWith(prefix) || request.after.length > 256)
    )
        return fail('invalid-cursor');
    const page = await kv.list({
        prefix,
        after: request.after || undefined,
        limit: LIMITS.pageSize,
        valuesIncluded: false,
    });
    const deleted = new Map<string, boolean>(),
        entries = [];
    if (
        request.exclude &&
        (!Array.isArray(request.exclude) ||
            request.exclude.length > LIMITS.documents ||
            !request.exclude.every(validId))
    )
        return fail('invalid-exclusions');
    const excluded = new Set<string>(request.exclude ?? []);
    for (const { key } of page.entries) {
        const document = key.split('/')[5];
        if (excluded.has(document)) continue;
        if (!deleted.has(document)) deleted.set(document, !!(await kv.get(deletedKey(grant.collection, document))));
        if (!deleted.get(document)) entries.push({ key });
    }
    return json(200, {
        entries,
        cursor: page.cursor,
        complete: page.entries.length < LIMITS.pageSize,
        owner: collection.owner,
        ...(request.directory === true ? { directory: await readDirectory(deleted) } : {}),
    });
}
/**
 * Returns one checkpoint with its ciphertext, the grant of its author, and the owner grant.
 */
export async function readCheckpoint(session: MemberSession) {
    const { kv, request, collection, grant } = session;

    const prefix = updatesPrefix(grant.collection, grant.document ?? '');
    if (typeof request.key !== 'string' || !request.key.startsWith(prefix) || request.key.length > 256)
        return fail('forbidden-checkpoint', 403);
    const stored = await read<Stored>(kv, request.key);
    if (stored && (await kv.get(deletedKey(grant.collection, stored.envelope.checkpoint.document))))
        return fail('document-deleted', 410);
    if (stored?.payload) {
        const ciphertext = await kv.get(stored.payload);
        if (!ciphertext) return json(404, { error: 'checkpoint-unavailable' });
        stored.envelope.checkpoint.ciphertext = ciphertext;
    }
    return stored
        ? json(200, { envelope: stored.envelope, author: stored.author, owner: collection.owner })
        : json(404, { error: 'checkpoint-unavailable' });
}
/**
 * Stores one signed checkpoint: checks epoch, permission, and signature, reserves capacity, writes the ciphertext and
 * then its header, and prunes older checkpoints of the same stream. A retry of the same operation returns the same
 * receipt.
 */
export async function appendCheckpoint(session: MemberSession) {
    const { runtime, kv, request, member, grant, scope } = session;

    const envelope = request.envelope;
    if (!validCheckpoint(envelope)) return fail('invalid-checkpoint');
    const c = envelope.checkpoint;
    scope(c.document);
    if (await kv.get(deletedKey(grant.collection, c.document))) return fail('document-deleted', 410);
    const epoch = await kv.list({
        prefix: epochsPrefix(grant.collection, c.document),
        limit: 1,
        valuesIncluded: true,
    });
    const currentEpoch = epoch.entries[0]?.value ? parse<{ epoch: number }>(epoch.entries[0].value).epoch : 1;
    if (c.epoch !== currentEpoch) return fail('key-epoch-changed', 409);
    if (c.collection !== grant.collection || c.member !== grant.member || !permitted(grant, c.document, c.stream))
        return fail('forbidden-stream', 403);
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
    const payload = payloadKey(key),
        expires = clock() + LIMITS.inactiveSeconds;
    await first(kv, payload, takeCiphertext(c), LIMITS.inactiveSeconds, runtime.crypto);
    const stored: Stored = {
        envelope: { ...envelope, checkpoint: { ...c, ciphertext: '' } },
        author: member.signed,
        slots,
        payload,
    };
    await first(kv, key, b64json(stored), Math.max(1, expires - clock()));
    // Recheck membership after storage. A concurrently revoked sender keeps its local outbox.
    if (await kv.get(deletedKey(grant.collection, c.document))) {
        await kv.delete(payload);
        await kv.delete(key);
        return fail('document-deleted', 410);
    }
    if (
        (await kv.get(grantKey(grant.collection, grant.member))) !== b64json(member) ||
        (await kv.get(revokedKey(grant.collection, grant.member)))
    )
        return fail('access-ended', 403);
    await first(kv, receiptKey, b64json({ digest }));
    await kv.delete(intentKey);
    // Keep the newest two checkpoints for this writer/stream. Each full body checkpoint
    // contains all previous local operations. A crash before pruning only wastes capacity.
    const older = await kv.list({ prefix, after: key, limit: 3, valuesIncluded: false });
    for (const entry of older.entries.slice(1)) {
        const old = await read<Stored>(kv, entry.key);
        // Large releases are split by authenticated maintenance, rather than
        // overrunning the invocation budget after a successful durable append.
        if (old && old.slots.length <= 8 && (await kv.delete(entry.key))) {
            if (old.payload) await kv.delete(old.payload);
            await release(kv, old.slots, old.envelope.checkpoint.operation);
        }
    }
    return json(200, { operation: c.operation, digest });
}
