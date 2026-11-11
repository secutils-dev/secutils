import assert from 'node:assert/strict';
import { test } from 'node:test';
import { DurableBody, type DurableRecord, type ReplicaRepository } from '../src/sync/durable.ts';
import { identity, keyBytes, checkpointDigest, tokenHash, randomSecret, sealCheckpoint } from '../src/sync/crypto.ts';
import { type Grant, type SyncBinding } from '../src/sync/protocol.ts';
import { BodyReplica } from '../src/sync/replica.ts';

class Repository implements ReplicaRepository {
    value: DurableRecord | undefined;
    fail = false;
    async get() {
        return this.value ? structuredClone(this.value) : undefined;
    }
    async commit(expected: number | null, value: DurableRecord) {
        if (this.fail) throw new Error('quota exceeded');
        if ((this.value?.revision ?? null) !== expected) throw new Error('revision conflict');
        this.value = structuredClone(value);
    }
}
const id = () => crypto.randomUUID();
async function fixture(permission: SyncBinding['permission'] = 'owner') {
    const repository = new Repository(),
        who = await identity(),
        key = keyBytes();
    const binding: SyncBinding = { collection: id(), document: id(), member: id(), epoch: 1, permission, kind: 'sync' };
    let client: DurableBody;
    if (permission === 'owner') client = await DurableBody.create(binding, 'one two three', repository, who, key);
    else {
        const seed = new BodyReplica();
        seed.seed('one two three');
        client = await DurableBody.join(binding, seed.checkpoint(), repository, who, key);
        seed.destroy();
    }
    return { repository, who, key, binding, client };
}
test('offline edits commit before encryption and a crash retries the exact persisted envelope', async () => {
    const f = await fixture();
    await f.client.edit({ from: 0, to: 0, insert: 'offline ' });
    assert.equal(f.repository.value?.projection, 'offline one two three');
    assert.equal(f.repository.value?.outbox[0].envelope, null);
    const prepared = await f.client.prepare();
    assert.ok(prepared);
    const reopened = new DurableBody((await f.repository.get())!, f.repository, f.who, f.key);
    assert.deepEqual(await reopened.prepare(), prepared);
    await assert.rejects(reopened.acknowledge(prepared.checkpoint.operation, 'wrong digest'), /mismatch/);
    assert.equal(f.repository.value?.outbox.length, 1);
    await reopened.acknowledge(prepared.checkpoint.operation, await checkpointDigest(prepared));
    assert.equal(f.repository.value?.outbox.length, 0);
});
test('unsent intents coalesce, while an already prepared ciphertext remains immutable', async () => {
    const f = await fixture();
    await f.client.edit({ from: 0, to: 0, insert: 'A' });
    await f.client.edit({ from: 0, to: 0, insert: 'B' });
    assert.equal(f.repository.value?.outbox.length, 1);
    const prepared = await f.client.prepare();
    await f.client.edit({ from: 0, to: 0, insert: 'C' });
    await f.client.edit({ from: 0, to: 0, insert: 'D' });
    assert.equal(f.repository.value?.outbox.length, 2);
    assert.deepEqual(await f.client.prepare(), prepared);
    assert.equal(f.repository.value?.projection, 'DCBAone two three');
});
test('a rejected remote write cannot advance progress or replace the live editor projection', async () => {
    const f = await fixture(),
        peer = await identity(),
        remote = new BodyReplica(f.client.body.checkpoint());
    remote.edit({ from: 0, to: 0, insert: 'remote ' });
    const author: Grant = {
        v: 1,
        collection: f.binding.collection,
        member: id(),
        participant: id(),
        permission: 'edit',
        document: null,
        publicKey: peer.publicKey,
        boxKey: peer.boxKey,
        expires: 0,
        tokenHash: await tokenHash(randomSecret()),
    };
    const envelope = await sealCheckpoint(
        {
            v: 1,
            collection: f.binding.collection,
            document: f.binding.document,
            member: author.member,
            stream: 'body',
            epoch: 1,
            sequence: 1,
            operation: id(),
        },
        remote.checkpoint(),
        f.key,
        peer,
    );
    f.repository.fail = true;
    await assert.rejects(f.client.receive(envelope, author), /quota/);
    assert.equal(f.client.body.text(), 'one two three');
    assert.deepEqual(f.repository.value?.progress, {});
    f.repository.fail = false;
    assert.equal(await f.client.receive(envelope, author), 'remote one two three');
    assert.equal(f.repository.value?.progress[`${author.member}/body`], 1);
    const commenter = { ...author, permission: 'comment' as const };
    await assert.rejects(f.client.receive(envelope, commenter), /unauthorized/);
});
test('read-only replicas cannot publish body changes or undo', async () => {
    const f = await fixture('comment');
    await assert.rejects(f.client.edit({ from: 0, to: 0, insert: 'forbidden' }), /read-only/);
    await assert.rejects(f.client.undo(), /read-only/);
    assert.equal(f.repository.value?.outbox.length, 0);
});
test('a stale or detached binding cannot prepare an upload', async () => {
    const f = await fixture();
    await f.client.edit({ from: 0, to: 0, insert: 'local ' });
    f.repository.value = undefined;
    await assert.rejects(f.client.prepare(), /detached/);
});
