import assert from 'node:assert/strict';
import { test } from 'node:test';
import { Session, type SessionRecord } from '../src/sync/session.ts';
import { identity, randomSecret, tokenHash, signGrant, hash, checkpointDigest, sealCheckpoint } from '../src/sync/crypto.ts';
import { decode, encode } from '../src/sync/protocol.ts';
import { BodyReplica } from '../src/sync/replica.ts';
import { joinComments, splitComments } from '../src/share/merge.ts';

async function fixture(text = '# Document\n\nBody.') {
    const who = await identity(), token = randomSecret(), collection = crypto.randomUUID(), document = crypto.randomUUID();
    const owner = await signGrant({ v: 1, collection, member: crypto.randomUUID(), participant: crypto.randomUUID(), permission: 'owner', document: null,
        publicKey: who.publicKey, boxKey: who.boxKey, tokenHash: await tokenHash(token), expires: 0 }, who);
    const record = Session.seed({ collection, document, member: owner.grant.member, permission: 'owner', kind: 'sync', epoch: 1 }, text, who, token, randomSecret(), owner, 'Document');
    let stored = structuredClone(record);
    const repository = { get: async () => structuredClone(stored), commit: async (expected: number | null, next: SessionRecord) => { assert.equal(expected, stored.revision); stored = structuredClone(next); } };
    return { who, token, owner, record, repository, session: new Session(record, repository) };
}
async function transfer(sender: Session, receiver: Session) {
    while (true) {
        const envelope = await sender.prepare(); if (!envelope) break;
        await receiver.receive(envelope, sender.record.member);
        await sender.acknowledge(envelope.checkpoint.operation, await checkpointDigest(envelope));
    }
}
async function join(f: Awaited<ReturnType<typeof fixture>>, permission: 'edit' | 'comment' = 'edit') {
    const who = await identity(), token = randomSecret(), memberId = crypto.randomUUID();
    const member = await signGrant({ ...f.owner.grant, member: memberId, participant: permission === 'comment' ? memberId : f.owner.grant.participant,
        permission, document: permission === 'comment' ? f.record.binding.document : null, publicKey: who.publicKey, boxKey: who.boxKey, tokenHash: await tokenHash(token) }, f.who);
    const record = structuredClone(f.record), empty = new BodyReplica();
    record.body = encode(empty.checkpoint()); empty.destroy(); record.member = member; record.identity = who; record.token = token;
    record.binding.member = member.grant.member; record.binding.permission = permission; record.seed = []; record.suffix = ''; record.projection = '';
    let stored = structuredClone(record);
    const repository = { get: async () => structuredClone(stored), commit: async (expected: number | null, next: SessionRecord) => { assert.equal(expected, stored.revision); stored = structuredClone(next); } };
    return new Session(record, repository);
}

test('joining adopts the creator state without adding an independent content seed', async () => {
    const f = await fixture(), peer = await join(f);
    await f.session.initial(); await transfer(f.session, peer); peer.present(peer.record.projection);
    await peer.change(peer.record.projection, '# Document\n\nOnly the replacement.'); await transfer(peer, f.session);
    assert.equal(f.session.record.projection, '# Document\n\nOnly the replacement.');
    assert.equal(f.session.replica.text().split('# Document').length, 2);
});

test('same-profile tabs can acknowledge the same uploaded packet without stopping Sync', async () => {
    const f = await fixture(); await f.session.initial();
    const other = new Session(f.session.record, f.repository);
    try {
        const packet = await f.session.prepare(); assert.ok(packet);
        assert.deepEqual(await other.prepare(), packet);
        const digest = await checkpointDigest(packet);
        await f.session.acknowledge(packet.checkpoint.operation, digest);
        await other.acknowledge(packet.checkpoint.operation, digest);
        assert.deepEqual(other.record.outbox, f.session.record.outbox);
        await assert.rejects(other.acknowledge(packet.checkpoint.operation, 'invalid digest'), /acknowledgement-mismatch/);
    } finally { other.destroy(); f.session.destroy(); }
});

test('own upload acknowledgements save received progress so a reopened client never needs its own echo', async () => {
    const f = await fixture();
    try {
        await f.session.initial();
        for (const stream of ['body', 'review']) {
            const packet = (await f.session.prepare())!; assert.equal(packet.checkpoint.stream, stream);
            await f.session.acknowledge(packet.checkpoint.operation, await checkpointDigest(packet));
            const record = await f.repository.get(), cursor = `${f.owner.grant.member}/${stream}`;
            assert.equal(record.progress[cursor], packet.checkpoint.sequence);
            const revision = record.revision; await f.session.receive(packet, f.owner);
            assert.equal((await f.repository.get()).revision, revision);
        }
        const reopened = new Session(await f.repository.get(), f.repository);
        try { assert.equal(await reopened.pending(), 0); assert.equal(Object.keys(reopened.record.progress).length, 2); }
        finally { reopened.destroy(); }
    } finally { f.session.destroy(); }
});

test('alternating owner/commenter replies observe the other author and preserve order after checkpoints and reopen', async () => {
    const f = await fixture('Body.'), peer = await join(f, 'comment'), threadId = `${f.owner.grant.participant}:${crypto.randomUUID()}`;
    await f.session.initial(); await transfer(f.session, peer);
    let preceding: string | undefined;
    try {
        for (const [index, sender] of [f.session, peer, f.session, peer].entries()) {
            const receiver = sender === peer ? f.session : peer;
            sender.present(sender.record.projection);
            const parsed = splitComments(sender.record.projection), thread = parsed.threads[0] ?? { id: threadId, quote: 'Body', comments: [] };
            const comment = { id: `${sender.record.member.grant.participant}:${crypto.randomUUID()}`, author: index % 2 ? 'B' : 'A', body: String(index + 1), at: '2026-11-08T12:00:00.000Z' };
            await sender.change(sender.record.projection, joinComments(parsed.body, [{ ...thread, comments: [...thread.comments, comment] }]));
            const operation = sender.record.reviews[sender.record.member.grant.member].operations.at(-1)!.operation;
            assert.equal(operation.kind, 'comment');
            if (preceding) assert.ok(operation.previous.includes(preceding), 'A reply must observe the preceding synced comment from the other author');
            preceding = operation.id;
            await transfer(sender, receiver);
            for (const session of [f.session, peer]) assert.deepEqual(splitComments(session.record.projection).threads[0].comments.map(c => c.body), Array.from({ length: index + 1 }, (_, n) => String(n + 1)));
        }
        const reopened = new Session(peer.record, peer.repository);
        try { await reopened.synchronize(); assert.deepEqual(splitComments(reopened.record.projection).threads[0].comments.map(c => c.body), ['1', '2', '3', '4']); }
        finally { reopened.destroy(); }
        const parsed = splitComments(peer.record.projection);
        let legacy = structuredClone(peer.record);
        legacy.projection = joinComments(parsed.body, [{ ...parsed.threads[0], comments: [parsed.threads[0].comments[0], parsed.threads[0].comments[1], parsed.threads[0].comments[3], parsed.threads[0].comments[2]] }]);
        const beforeRepair = structuredClone(legacy);
        const repair = new Session(legacy, { get: async () => structuredClone(legacy), commit: async (expected, value) => { assert.equal(expected, legacy.revision); legacy = structuredClone(value); } });
        try {
            assert.deepEqual(splitComments(await repair.synchronize()).threads[0].comments.map(c => c.body), ['1', '2', '3', '4']);
            assert.deepEqual(legacy.reviews, beforeRepair.reviews);
            assert.deepEqual(legacy.outbox, beforeRepair.outbox);
            assert.deepEqual(legacy.progress, beforeRepair.progress);
            assert.equal(legacy.nextSequence, beforeRepair.nextSequence);
            const revision = legacy.revision; await repair.synchronize(); assert.equal(legacy.revision, revision);
        } finally { repair.destroy(); }
        assert.deepEqual(splitComments(f.session.record.projection).threads[0].comments.map(c => c.author), ['A', 'B', 'A', 'B']);
        for (const session of [f.session, peer]) for (const packet of Object.values(session.record.reviews)) assert.ok(packet.operations.every(op => op.operation.actor === packet.grant.grant.member));
    } finally { peer.destroy(); f.session.destroy(); }
});

test('a keystroke staged while a remote update commits preserves both edits', async () => {
    const f = await fixture('one two three'), peer = await join(f);
    await f.session.initial(); await transfer(f.session, peer); peer.present(peer.record.projection);
    await peer.change(peer.record.projection, 'remote one two three');
    const envelope = await peer.prepare(); assert.ok(envelope);
    const incoming = f.session.receive(envelope, peer.record.member);
    const local = f.session.change('one two three', 'one two three local');
    await incoming; await local;
    assert.equal(f.session.replica.text(), 'remote one two three local');
    await f.session.undo(); assert.equal(f.session.replica.text(), 'remote one two three');
});

test('review checkpoints stay author-separated and retain concurrent versions', async () => {
    const f = await fixture('# Document\n\nBody.'), peer = await join(f);
    await f.session.initial(); await transfer(f.session, peer); peer.present(peer.record.projection);
    const id = `${f.owner.grant.participant}:${crypto.randomUUID()}`, thread = { id: `${f.owner.grant.participant}:${crypto.randomUUID()}`, quote: 'Body', comments: [{ id, author: 'Owner', body: 'Original' }] };
    const original = joinComments('# Document\n\nBody.', [thread]);
    await f.session.change(f.session.record.projection, original); await transfer(f.session, peer);
    f.session.present(f.session.record.projection); peer.present(peer.record.projection);
    const modified = (body: string) => joinComments(splitComments(original).body, [{ ...thread, comments: [{ ...thread.comments[0], body }] }]);
    await Promise.all([f.session.change(original, modified('Version A')), peer.change(original, modified('Version B'))]);
    await transfer(peer, f.session); await transfer(f.session, peer);
    assert.deepEqual(splitComments(f.session.record.projection).threads[0].comments.map((c) => c.body).sort(), ['Version A', 'Version B']);
    assert.deepEqual(splitComments(peer.record.projection).threads[0].comments.map((c) => c.body).sort(), ['Version A', 'Version B']);
});

test('Undo restores an editor-moderated comment and thread with their original authors and bytes', async () => {
    const seed = [{ id: 'historical-thread', quote: 'Body', comments: [{ id: 'historical-comment', author: 'Original author', body: 'Unchanged original comment', at: '2026-11-01' }] }];
    const f = await fixture(joinComments('Body.', seed)), peer = await join(f);
    // Give this editor a distinct review identity, as Live links do.
    const member = await signGrant({ ...peer.record.member.grant, participant: peer.record.member.grant.member }, f.who);
    const initial = { ...peer.record, member };
    let stored = structuredClone(initial);
    const editor = new Session(initial, { get: async () => structuredClone(stored), commit: async (expected, value) => { assert.equal(expected, stored.revision); stored = structuredClone(value); } });
    await f.session.initial(); await transfer(f.session, editor); editor.present(editor.record.projection);
    const deleted = [{ ...seed[0], comments: [] }];
    await editor.change(editor.record.projection, joinComments('Body.', deleted)); await transfer(editor, f.session);
    assert.equal(splitComments(f.session.record.projection).threads[0].comments.length, 0);
    await editor.undoReview(seed, deleted); await transfer(editor, f.session);
    assert.deepEqual(splitComments(f.session.record.projection).threads, seed);
    editor.present(editor.record.projection);
    await editor.change(editor.record.projection, joinComments('Body.', [])); await transfer(editor, f.session);
    await editor.undoReview(seed, []); await transfer(editor, f.session);
    assert.deepEqual(splitComments(f.session.record.projection).threads, seed);
    assert.ok(editor.record.reviews[member.grant.member].operations.some(({ operation }) => operation.kind === 'undo-delete'));
});

test('same-profile replica conflicts retry against durable state without losing either edit', async () => {
    const f = await fixture('one two three');
    let stored = structuredClone(f.record);
    const repository = { get: async () => structuredClone(stored), commit: async (expected: number | null, next: SessionRecord) => {
        if (stored.revision !== expected) { const error = new Error('Stale replica'); error.name = 'ReplicaConflict'; throw error; }
        stored = structuredClone(next);
    } };
    const first = new Session(f.record, repository), second = new Session(f.record, repository);
    await Promise.all([first.change('one two three', 'first one two three'), second.change('one two three', 'one two three second')]);
    await first.synchronize(); await second.synchronize();
    assert.equal(first.record.projection, 'first one two three second');
    assert.equal(second.record.projection, first.record.projection);
    assert.equal(stored.outbox.length, 1);
});

test('review acknowledgements bind the participant and hash the latest durable body', async () => {
    const f = await fixture('before');
    const stale = new Session(f.record, f.repository);
    await f.session.change('before', 'after');
    await stale.markReviewed();
    const ack = stale.record.reviews[f.owner.grant.member].operations.at(-1)!.operation;
    assert.equal(ack.kind, 'ack');
    assert.equal(ack.participant, f.owner.grant.participant);
    assert.equal(ack.thread, `ack:${f.owner.grant.participant}`);
    assert.equal(ack.value, await hash(new TextEncoder().encode('after')));
});

test('an authorized body checkpoint cannot hide or impersonate author-separated reviews through a Markdown marker', async () => {
    const trusted = { id: 'legacy-thread', quote: 'Body', comments: [{ id: 'legacy-comment', author: 'Owner', body: 'Trusted review' }] };
    for (const seed of [[], [trusted]]) {
        const f = await fixture(joinComments('Body.', seed)), peer = await join(f);
        await f.session.initial(); await transfer(f.session, peer);
        const injected = joinComments('Editable body.', [{ id: 'forged-thread', quote: 'Editable', comments: [{ id: 'forged-comment', author: 'Impersonated author', body: 'Forged review' }] }]);
        peer.replica.replace(injected);
        const envelope = await sealCheckpoint({ v: 1, collection: peer.record.binding.collection, document: peer.record.binding.document, member: peer.record.binding.member,
            stream: 'body', sequence: 1, operation: crypto.randomUUID(), epoch: 1 }, peer.replica.checkpoint(), decode(peer.record.key), peer.record.identity);
        await f.session.receive(envelope, peer.record.member);
        assert.deepEqual(splitComments(f.session.record.projection).threads, seed);
        assert.equal(splitComments(f.session.record.projection).body.includes('Forged review'), true);
    }
});

test('resuming an already applied rotation preserves the exact prepared checkpoint', async () => {
    const f = await fixture(); await f.session.initial();
    const key = randomSecret(); await f.session.rekey(2, key, true);
    const prepared = await f.session.prepare(); assert.ok(prepared);
    await f.session.rekey(2, key, true);
    assert.deepEqual(await f.session.prepare(), prepared);
});
