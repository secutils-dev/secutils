import assert from 'node:assert/strict';
import { test } from 'node:test';
import { resumeRotation, type RotationPlan } from '../src/sync/rotation.ts';

function fixture() {
    let stored: RotationPlan | null = { v: 1, id: crypto.randomUUID(), revision: 1, collection: crypto.randomUUID(), member: crypto.randomUUID(),
        target: { invitation: crypto.randomUUID() }, documents: [{ document: crypto.randomUUID(), epoch: 2, key: 'retained private key', values: [],
            links: [{ invitation: crypto.randomUUID(), value: { document: crypto.randomUUID(), epoch: 2, nonce: 'immutable nonce', ciphertext: 'immutable ciphertext' }, signature: 'immutable signature' }] }] };
    const calls: { action: string; value: string }[] = [], applied: string[] = [];
    const repository = { get: async () => structuredClone(stored), commit: async (expected: number | null, value: RotationPlan | null) => {
        assert.equal(expected, stored?.revision ?? null); stored = structuredClone(value);
    } };
    return { repository, calls, applied, stored: () => structuredClone(stored) };
}
for (const failedAction of ['invite-revoke', 'live-key-write', 'rotate-document']) test(`interrupted ${failedAction} resumes the same durable key and ciphertext after a lost acknowledgement`, async () => {
    const f = fixture(); let fail = true;
    const request = async (action: string, value: Record<string, unknown>) => {
        f.calls.push({ action, value: JSON.stringify(value) });
        if (action === failedAction && fail) { fail = false; throw new Error('Lost service acknowledgement'); }
    };
    const apply = async (value: { key: string }) => { f.applied.push(value.key); };
    await assert.rejects(resumeRotation(f.repository, request, apply));
    assert.equal(f.stored()?.documents[0].key, 'retained private key');
    await resumeRotation(f.repository, request, apply);
    const attempts = f.calls.filter((call) => call.action === failedAction);
    assert.equal(attempts.length, 2); assert.equal(attempts[0].value, attempts[1].value);
    assert.deepEqual(f.applied, ['retained private key']); assert.equal(f.stored(), null);
});
test('a failed local apply leaves the activated rotation recoverable and does not rewrite distributed envelopes', async () => {
    const f = fixture(); let fail = true;
    const request = async (action: string, value: Record<string, unknown>) => { f.calls.push({ action, value: JSON.stringify(value) }); };
    const apply = async () => { if (fail) { fail = false; throw new Error('Storage unavailable'); } };
    await assert.rejects(resumeRotation(f.repository, request, apply));
    assert.equal(f.stored()?.documents[0].activated, true);
    const count = f.calls.length;
    await resumeRotation(f.repository, request, apply);
    assert.equal(f.calls.length, count); assert.equal(f.stored(), null);
});
test('a lost journal-clear commit retries only the clear, not another rotation', async () => {
    const f = fixture(), original = f.repository.commit; let fail = true;
    f.repository.commit = async (expected, value) => { if (value === null && fail) { fail = false; throw new Error('Storage unavailable'); } await original(expected, value); };
    const request = async (action: string, value: Record<string, unknown>) => { f.calls.push({ action, value: JSON.stringify(value) }); };
    const apply = async (value: { key: string }) => { f.applied.push(value.key); };
    await assert.rejects(resumeRotation(f.repository, request, apply));
    assert.equal(f.stored()?.documents[0].applied, true); const count = f.calls.length;
    await resumeRotation(f.repository, request, apply);
    assert.equal(f.calls.length, count); assert.deepEqual(f.applied, ['retained private key']); assert.equal(f.stored(), null);
});
