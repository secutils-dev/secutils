import assert from 'node:assert/strict';
import { test } from 'node:test';
import { BodyReplica, sourceChange } from '../src/sync/replica.ts';
import { decode, encode, permitted, type Grant } from '../src/sync/protocol.ts';
import { identity, keyBytes, openCheckpoint, pairingPhrase, sealCheckpoint, signGrant, tokenHash, randomSecret, unwrapKey, verify, wrapKey, encryptRecovery, decryptRecovery } from '../src/sync/crypto.ts';
import { ReviewReplica, signReview, validReview, type ReviewOperation } from '../src/sync/reviews.ts';
import { responderInternals } from '../src/sync/responder.ts';

const id = () => crypto.randomUUID();
const grantFor = async (who: Awaited<ReturnType<typeof identity>>, permission: Grant['permission'] = 'edit', collection = id()): Promise<Grant> => ({
    v: 1, collection, member: id(), participant: id(), permission, document: null, publicKey: who.publicKey, boxKey: who.boxKey, tokenHash: await tokenHash(randomSecret()), expires: 0,
});

test('body replicas seed once, preserve bytes, and converge after concurrent, reordered, and duplicate checkpoints', () => {
    const source = '\uFEFF---\r\ntitle: 日本語\r\n---\r\n\r\n| A | B |\r\n| :--- | ---: |\r\n| a \\| b | 😀 |\r\n<!-- su-table {"widths":[0.3,0.7]} -->\r\n';
    const creator = new BodyReplica(); creator.seed(source);
    assert.throws(() => creator.seed('second seed'), /already-seeded/);
    const a = new BodyReplica(creator.checkpoint()), b = new BodyReplica(creator.checkpoint());
    a.edit({ from: 0, to: 0, insert: 'A\n' });
    b.edit({ from: source.length, to: source.length, insert: '\nB' });
    const one = a.checkpoint(), two = b.checkpoint();
    a.apply(two); a.apply(two); b.apply(one); b.apply(two); b.apply(one);
    assert.equal(a.text(), b.text()); assert.equal(a.text(), `A\n${source}\nB`);
    const join = new BodyReplica(a.checkpoint()); assert.equal(join.text(), a.text());
    for (const replica of [creator, a, b, join]) replica.destroy();
});

test('local undo never undoes an incoming operation and anchors follow unrelated insertions', () => {
    const a = new BodyReplica(); a.seed('one two three');
    const anchor = a.anchor(4, 7), b = new BodyReplica(a.checkpoint());
    a.edit({ from: 4, to: 7, insert: 'TWO' });
    b.edit({ from: 0, to: 0, insert: 'remote ' });
    a.apply(b.checkpoint()); a.undo.undo();
    assert.equal(a.text(), 'remote one two three');
    assert.deepEqual(a.range(anchor), [11, 14]);
});

test('minimal source splices retain UTF-16 pairs and untouched source bytes', () => {
    for (const [before, after] of [['a😀z', 'a😄z'], ['abc', 'abc!'], ['abc', 'ac'], ['', 'text'], ['same', 'same']]) {
        const change = sourceChange(before, after);
        assert.equal(change ? before.slice(0, change.from) + change.insert + before.slice(change.to) : before, after);
    }
});

test('empty documents still seed once and overlapping draft targets require recovery', () => {
    const empty = new BodyReplica(); empty.seed('');
    assert.throws(() => empty.seed('second seed'), /already-seeded/);
    const joined = new BodyReplica(empty.checkpoint());
    assert.throws(() => joined.seed('independent seed'), /already-seeded/);
    const body = new BodyReplica(); body.seed('one two three');
    const anchor = body.anchor(4, 7), peer = new BodyReplica(body.checkpoint());
    peer.edit({ from: 0, to: 0, insert: 'prefix ' }); body.apply(peer.checkpoint());
    assert.equal(body.unchanged(anchor), true);
    peer.edit({ from: 11, to: 14, insert: 'changed' }); body.apply(peer.checkpoint());
    assert.equal(body.unchanged(anchor), false);
});

test('native Web Crypto signatures verify with the responder verifier, including high-S signatures', async () => {
    const who = await identity(), grant = await grantFor(who), signed = await signGrant(grant, who);
    assert.equal(await verify(grant, signed.signature, who.publicKey), true);
    assert.equal(responderInternals.signatureValid(grant, signed.signature, who.publicKey), true);
    assert.equal(responderInternals.signatureValid({ ...grant, permission: 'owner' }, signed.signature, who.publicKey), false);
});

test('checkpoints bind document, member, sequence, stream, epoch, and exact ciphertext', async () => {
    const who = await identity(), grant = await grantFor(who), document = id(), key = keyBytes();
    const metadata = { v: 1 as const, collection: grant.collection, document, member: grant.member, stream: 'body' as const, epoch: 1, sequence: 1, operation: id() };
    const envelope = await sealCheckpoint(metadata, new TextEncoder().encode('private 日本語'), key, who);
    assert.equal(new TextDecoder().decode(await openCheckpoint(envelope, key, grant)), 'private 日本語');
    for (const property of ['document', 'member', 'stream', 'epoch', 'sequence', 'ciphertext'] as const) {
        const altered = structuredClone(envelope);
        (altered.checkpoint as unknown as Record<string, unknown>)[property] = property === 'epoch' || property === 'sequence' ? 2 : property === 'stream' ? 'review' : id();
        await assert.rejects(openCheckpoint(altered, key, grant), /invalid-checkpoint/);
    }
    await assert.rejects(openCheckpoint(envelope, keyBytes(), grant), /encryption/);
});

test('per-document key wrapping binds the recipient and key epoch without exposing the key', async () => {
    const sender = await identity(), receiver = await identity(), wrong = await identity(), key = keyBytes();
    const wrapped = await wrapKey({ collection: id(), document: id(), epoch: 1, member: id() }, key, sender, receiver.boxKey);
    assert.deepEqual(await unwrapKey(wrapped, receiver), key);
    await assert.rejects(unwrapKey(wrapped, wrong), /key-envelope/);
    await assert.rejects(unwrapKey({ ...wrapped, epoch: 2 }, receiver), /key-envelope/);
    assert.equal(JSON.stringify(wrapped).includes(encode(key)), false);
});

test('pairing phrase binds both scope and requested keys', async () => {
    const who = await identity(), grant = await grantFor(who), invitation = { id: id(), document: id() };
    const phrase = await pairingPhrase(invitation, grant);
    assert.match(phrase, /^[a-f0-9]{4} [a-f0-9]{4} [a-f0-9]{4}$/);
    assert.notEqual(await pairingPhrase({ ...invitation, document: id() }, grant), phrase);
    assert.notEqual(await pairingPhrase(invitation, { ...grant, publicKey: (await identity()).publicKey }), phrase);
});

test('view/comment/edit grants have separate server stream permissions', async () => {
    const who = await identity(), document = id(), grant = await grantFor(who, 'comment');
    assert.equal(permitted(grant, document, 'body'), false);
    assert.equal(permitted(grant, document, 'review'), true);
    assert.equal(permitted({ ...grant, permission: 'view' }, document, 'review'), false);
    assert.equal(permitted({ ...grant, permission: 'edit', document }, id(), 'body'), false);
    assert.equal(permitted({ ...grant, permission: 'edit', document }, document, 'tree'), false);
});

test('typed reviews retain concurrent comment versions and reject another participant impersonating an author', async () => {
    const who = await identity(), other = await identity(), grant = await grantFor(who, 'comment'), otherGrant = await grantFor(other, 'comment', grant.collection);
    const document = id(), thread = `${grant.participant}:${id()}`, comment = `${grant.participant}:${id()}`, replica = new ReviewReplica(document);
    const base = { v: 1 as const, document, actor: grant.member, participant: grant.participant, previous: [] as string[], thread };
    const create: ReviewOperation = { ...base, id: id(), kind: 'thread', comment: null, value: { id: thread, quote: 'text', comments: [] } };
    await replica.apply(await signReview(create, who), grant);
    const first: ReviewOperation = { ...base, id: id(), kind: 'comment', comment, value: { id: comment, body: 'first', author: 'Alice' } };
    await replica.apply(await signReview(first, who), grant);
    for (const body of ['version A', 'version B']) {
        const edit: ReviewOperation = { ...first, id: id(), previous: [first.id], value: { id: comment, body, author: 'Alice' } };
        await replica.apply(await signReview(edit, who), grant);
    }
    assert.deepEqual(replica.threads()[0].comments.map((c) => c.body).sort(), ['version A', 'version B']);
    const forged: ReviewOperation = { ...first, id: id(), actor: otherGrant.member, participant: otherGrant.participant, previous: [first.id], value: { id: comment, body: 'forged' } };
    await assert.rejects(replica.apply(await signReview(forged, other), otherGrant), /author-mismatch/);
    // A fresh replica rejects the forgery too, before it sees the genuine first comment.
    await assert.rejects(new ReviewReplica(document).apply(await signReview(forged, other), otherGrant), /author-mismatch/);
    assert.equal(validReview({ ...first, bodyUpdate: 'opaque CRDT smuggling' }), false);
    assert.equal(validReview({ ...first, kind: 'body' }), false);
});

test('base64 codec rejects noncanonical encodings', () => {
    for (const value of ['日本語 😀', 'hello', 'a']) {
        const bytes = new TextEncoder().encode(value); assert.deepEqual(decode(encode(bytes)), bytes);
    }
    assert.throws(() => decode('AB'), /noncanonical/);
    assert.throws(() => decode('AA='), /base64/);
});

test('alternating comments retain causal order despite random IDs, delivery order, and clock skew', async () => {
    const a = await identity(), b = await identity(), owner = await grantFor(a, 'owner'), guest = await grantFor(b, 'comment', owner.collection);
    const document = id(), thread = `${owner.participant}:${id()}`, opId = (n: number) => `${n.toString(16).padStart(8, '0')}-0000-4000-8000-000000000001`;
    const create = await signReview({ v: 1, id: opId(0), document, actor: owner.member, participant: owner.participant, previous: [], kind: 'thread', thread, comment: null,
        value: { id: thread, quote: 'Body', comments: [] } }, a);
    for (const times of [
        ['2026-11-08T12:00:01.000Z', '2026-11-08T12:00:02.000Z', '2026-11-08T12:00:03.000Z', '2026-11-08T12:00:04.000Z'],
        Array(4).fill('2026-11-08T12:00:00.000Z'),
        ['2026-11-08T12:00:00.000Z', '2026-11-08T11:00:00.000Z', '2026-11-08T12:00:01.000Z', '2026-11-08T11:00:01.000Z'],
    ]) {
        const operations: Awaited<ReturnType<typeof signReview>>[] = [];
        for (const [index, number] of [1, 2, 9, 3].entries()) {
            const grant = index % 2 ? guest : owner, comment = `${grant.participant}:${opId(number)}`;
            operations.push(await signReview({ v: 1, id: opId(number), document, actor: grant.member, participant: grant.participant, thread, comment, kind: 'comment',
                previous: [operations.at(-1)?.operation.id ?? create.operation.id], value: { id: comment, author: index % 2 ? 'B' : 'A', body: String(index + 1), at: times[index] } }, index % 2 ? b : a));
        }
        for (const order of [[create, ...operations], [create, operations[0], operations[2], operations[1], operations[3]], [...operations].reverse().concat(create)]) {
            const replica = new ReviewReplica(document);
            for (const signed of order) await replica.apply(signed, signed.operation.actor === owner.member ? owner : guest);
            await replica.apply(operations[3], guest);
            assert.deepEqual(replica.threads()[0].comments.map(c => c.body), ['1', '2', '3', '4']);
            const edited = await signReview({ ...operations[2].operation, id: opId(4), previous: [operations[2].operation.id], value: { ...(operations[2].operation.value as object), body: '3 edited', at: '1900-01-01' } }, a);
            await replica.apply(edited, owner);
            assert.deepEqual(replica.threads()[0].comments.map(c => c.body), ['1', '2', '3 edited', '4']);
        }
    }
});

test('legacy comments use creation dates, while seed comments retain their original positions through edit and Undo', async () => {
    const a = await identity(), b = await identity(), owner = await grantFor(a, 'owner'), guest = await grantFor(b, 'comment', owner.collection);
    const document = id(), thread = 'seed-thread', seed = [{ id: thread, quote: 'Body', comments: [
        { id: 'seed-1', author: 'A', body: '1', at: '2026-11-08T12:00:02.000Z' },
        { id: 'seed-2', author: 'A', body: '2', at: '2026-11-08T12:00:01.000Z' },
    ] }], replica = new ReviewReplica(document, seed, owner.participant);
    const base = { v: 1 as const, document, actor: owner.member, participant: owner.participant, previous: [] as string[], thread };
    await replica.apply(await signReview({ ...base, id: id(), kind: 'comment', comment: 'seed-1', value: { ...seed[0].comments[0], body: '1 edited' } }, a), owner);
    assert.deepEqual(replica.threads()[0].comments.map(c => c.body), ['1 edited', '2']);
    const deleted = await signReview({ ...base, id: id(), kind: 'delete-comment', comment: 'seed-1', value: null, previous: replica.heads(thread, 'seed-1', 'comment').map(op => op.operation.id) }, a);
    await replica.apply(deleted, owner);
    assert.deepEqual(replica.threads()[0].comments.map(c => c.body), ['2']);
    await replica.apply(await signReview({ ...base, id: id(), kind: 'undo-delete', comment: 'seed-1', value: deleted.operation.id, previous: [deleted.operation.id] }, a), owner);
    assert.deepEqual(replica.threads()[0].comments.map(c => c.body), ['1 edited', '2']);
    const legacy = new ReviewReplica(document, [{ id: thread, quote: 'Body', comments: [] }], owner.participant), operations: Awaited<ReturnType<typeof signReview>>[] = [];
    for (const [index, n] of [1, 2, 9, 3].entries()) {
        const grant = index % 2 ? guest : owner, comment = `${grant.participant}:${n}`, own = operations.filter(op => op.operation.actor === grant.member).at(-1);
        operations.push(await signReview({ ...base, actor: grant.member, participant: grant.participant, id: `${n.toString(16).padStart(8, '0')}-0000-4000-8000-000000000001`,
            kind: 'comment', comment, previous: own ? [own.operation.id] : [], value: { id: comment, author: index % 2 ? 'B' : 'A', body: String(index + 1), at: `2026-11-08T12:00:0${index + 1}.000Z` } }, index % 2 ? b : a));
    }
    for (const signed of operations.reverse()) await legacy.apply(signed, signed.operation.actor === owner.member ? owner : guest);
    assert.deepEqual(legacy.threads()[0].comments.map(c => c.body), ['1', '2', '3', '4']);
    const third = operations.find(op => (op.operation.value as { body: string }).body === '3')!;
    await legacy.apply(await signReview({ ...base, id: '00000000-0000-4000-8000-000000000001', kind: 'resolve', comment: null, value: true, previous: [third.operation.id] }, a), owner);
    assert.deepEqual(legacy.threads()[0].comments.map(c => c.body), ['1', '2', '3', '4']);
    await legacy.apply(await signReview({ ...third.operation, id: id(), previous: [third.operation.id], value: { ...(third.operation.value as object), at: '1900-01-01', body: '3 edited' } }, a), owner);
    assert.deepEqual(legacy.threads()[0].comments.map(c => c.body), ['1', '2', '3 edited', '4']);
});

test('edit-access moderation is actor-signed, while comment-only access cannot delete or impersonate other authors', async () => {
    const owner = await identity(), editor = await identity(), commenter = await identity();
    const ownerGrant = await grantFor(owner, 'owner'), editGrant = await grantFor(editor, 'edit', ownerGrant.collection), commentGrant = await grantFor(commenter, 'comment', ownerGrant.collection);
    const document = id(), thread = `${ownerGrant.participant}:${id()}`, comment = `${ownerGrant.participant}:${id()}`;
    const base = { v: 1 as const, document, actor: ownerGrant.member, participant: ownerGrant.participant, thread, previous: [] as string[] };
    const create = await signReview({ ...base, id: id(), kind: 'thread', comment: null, value: { id: thread, quote: 'text', comments: [] } }, owner);
    const first = await signReview({ ...base, id: id(), kind: 'comment', comment, previous: [create.operation.id], value: { id: comment, body: 'original', author: 'Owner' } }, owner);
    for (const historical of [false, true]) {
        const replica = new ReviewReplica(document, historical ? [{ id: thread, quote: 'text', comments: [{ id: comment, body: 'original', author: 'Owner' }] }] : [], ownerGrant.participant);
        if (!historical) { await replica.apply(create, ownerGrant); await replica.apply(first, ownerGrant); }
        const deletion: ReviewOperation = { ...base, id: id(), actor: commentGrant.member, participant: commentGrant.participant, kind: 'delete-comment', comment, value: null, previous: [first.operation.id] };
        await assert.rejects(replica.apply(await signReview(deletion, commenter), commentGrant), /author-mismatch|read-only/);
        await assert.rejects(replica.apply(await signReview({ ...deletion, id: id(), kind: 'delete-thread', comment: null }, commenter), commentGrant), /owner-required/);
        const moderation = await signReview({ ...deletion, id: id(), actor: editGrant.member, participant: editGrant.participant }, editor);
        await assert.rejects(replica.apply(await signReview({ ...moderation.operation, id: id(), kind: 'comment', value: { id: comment, body: 'impersonation' } }, editor), editGrant), /author-mismatch|read-only/);
        await replica.apply(moderation, editGrant);
        assert.equal(replica.threads()[0].comments.length, 0);
        assert.equal(replica.operations.get(moderation.operation.id)!.operation.actor, editGrant.member);
        await replica.apply(await signReview({ ...moderation.operation, id: id(), kind: 'delete-thread', comment: null }, editor), editGrant);
        assert.deepEqual(replica.threads(), []);
    }
    const fresh = new ReviewReplica(document);
    await assert.rejects(fresh.apply(await signReview({ ...base, id: id(), actor: commentGrant.member, participant: commentGrant.participant, kind: 'delete-comment', comment, value: null }, commenter), commentGrant), /author-mismatch/);
    const ownThread = `${commentGrant.participant}:${id()}`, ownComment = `${commentGrant.participant}:${id()}`;
    const ownBase = { ...base, thread: ownThread, actor: commentGrant.member, participant: commentGrant.participant };
    const ownCreate = await signReview({ ...ownBase, id: id(), kind: 'thread', comment: null, value: { id: ownThread, quote: 'text', comments: [] } }, commenter);
    const ownFirst = await signReview({ ...ownBase, id: id(), kind: 'comment', comment: ownComment, value: { id: ownComment, body: 'own comment' } }, commenter);
    await fresh.apply(ownCreate, commentGrant); await fresh.apply(ownFirst, commentGrant);
    const reply = await signReview({ ...base, id: id(), thread: ownThread, kind: 'comment', comment, value: { id: comment, body: 'other author reply' } }, owner);
    await fresh.apply(reply, ownerGrant);
    await assert.rejects(fresh.apply(await signReview({ ...ownBase, id: id(), kind: 'delete-thread', comment: null, value: null }, commenter), commentGrant), /other-authors/);
    await fresh.apply(await signReview({ ...ownBase, id: id(), kind: 'delete-comment', comment: ownComment, previous: [ownFirst.operation.id], value: null }, commenter), commentGrant);
    assert.deepEqual(fresh.threads()[0].comments.map(comment => comment.body), ['other author reply']);
    const deletion = await signReview({ ...base, id: id(), thread: ownThread, kind: 'delete-comment', comment: ownComment, value: null }, owner);
    await fresh.apply(deletion, ownerGrant);
    const forgedUndo = { ...ownBase, id: id(), kind: 'undo-delete' as const, comment: ownComment, value: deletion.operation.id, previous: [deletion.operation.id] };
    await assert.rejects(fresh.apply(await signReview(forgedUndo, commenter), commentGrant), /undo-not-owned/);
});
test('recovery packages encrypt credentials and reject the wrong password', async () => {
    const secret = randomSecret(), value = { token: secret, body: 'unpublished text', private: (await identity()).signing };
    const encrypted = await encryptRecovery(value, 'separate recovery password');
    assert.equal(JSON.stringify(encrypted).includes(secret), false);
    assert.equal(JSON.stringify(encrypted).includes('unpublished text'), false);
    assert.deepEqual(await decryptRecovery(encrypted, 'separate recovery password'), value);
    await assert.rejects(decryptRecovery(encrypted, 'wrong password'));
    await assert.rejects(encryptRecovery({ body: 'x'.repeat(12 * 1024 * 1024) }, 'separate recovery password'), /recovery-package-too-large/);
});

test('an offline reply to a concurrently deleted thread recovers the reply in either delivery order', async () => {
    const owner = await identity(), reviewer = await identity(), ownerGrant = await grantFor(owner, 'owner'), reviewerGrant = await grantFor(reviewer, 'comment', ownerGrant.collection);
    const document = id(), thread = `${ownerGrant.participant}:${id()}`, comment = `${ownerGrant.participant}:${id()}`;
    const base = { v: 1 as const, document, actor: ownerGrant.member, participant: ownerGrant.participant, thread, previous: [] as string[] };
    const create = await signReview({ ...base, id: id(), kind: 'thread', comment: null, value: { id: thread, quote: 'text', comments: [] } }, owner);
    const first = await signReview({ ...base, id: id(), kind: 'comment', comment, previous: [create.operation.id], value: { id: comment, body: 'known comment' } }, owner);
    const deleted = await signReview({ ...base, id: id(), kind: 'delete-thread', comment: null, value: null, previous: [first.operation.id] }, owner);
    const replyId = `${reviewerGrant.participant}:${id()}`;
    const reply = await signReview({ ...base, id: id(), actor: reviewerGrant.member, participant: reviewerGrant.participant, kind: 'comment', comment: replyId,
        previous: [first.operation.id], value: { id: replyId, body: 'offline reply' } }, reviewer);
    for (const order of [[deleted, reply], [reply, deleted]]) {
        const replica = new ReviewReplica(document);
        for (const operation of [create, first, ...order]) await replica.apply(operation, operation.operation.actor === ownerGrant.member ? ownerGrant : reviewerGrant);
        assert.deepEqual(replica.threads()[0].comments.map((c) => c.body), ['offline reply']);
        assert.equal(replica.recoveredThreads.has(thread), true);
    }
});
