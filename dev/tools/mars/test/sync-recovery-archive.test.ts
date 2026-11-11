import assert from 'node:assert/strict';
import { test } from 'node:test';
import { encryptRecoveryArchive, decryptRecoveryArchive, isRecoveryArchive } from '../src/sync/recovery-archive.ts';

const password = 'separate recovery password';
test('binary recovery exceeds the old 16 MiB limit while preserving credentials and pending work', async () => {
    const value = {
        key: 'private recovery credential',
        pending: ['unsent intent'],
        body: 'x'.repeat(17 * 1024 * 1024),
    };
    const file = await encryptRecoveryArchive(value, password);
    assert.ok(file.size > 16 * 1024 * 1024);
    assert.equal(await isRecoveryArchive(file), true);
    const prefix = await file.slice(0, 4096).text();
    assert.equal(prefix.includes(value.key), false);
    const recovered = await decryptRecoveryArchive<typeof value>(file, password);
    assert.equal(recovered.body.length, value.body.length);
    assert.equal(recovered.body, value.body);
    assert.equal(recovered.key, value.key);
    assert.deepEqual(recovered.pending, value.pending);
});
test('chunk boundaries retain Unicode and reject a wrong password, truncation, corruption, and reordered chunks', async () => {
    const value = { body: 'x'.repeat(65524) + '🚀漢字Δ'.repeat(16000), token: 'private token' };
    const file = await encryptRecoveryArchive(value, password, 65536);
    assert.deepEqual(await decryptRecoveryArchive(file, password), value);
    await assert.rejects(decryptRecoveryArchive(file, 'wrong recovery password'));
    await assert.rejects(decryptRecoveryArchive(file.slice(0, file.size - 1), password), /invalid-recovery-archive/);
    const bytes = new Uint8Array(await file.arrayBuffer()),
        start = 12 + new DataView(bytes.buffer).getUint32(8, true),
        chunk = 65536 + 16;
    const corrupt = bytes.slice();
    corrupt[start + 10] ^= 1;
    await assert.rejects(decryptRecoveryArchive(new Blob([corrupt]), password));
    const swapped = new Blob([
        bytes.slice(0, start),
        bytes.slice(start + chunk, start + 2 * chunk),
        bytes.slice(start, start + chunk),
        bytes.slice(start + 2 * chunk),
    ]);
    await assert.rejects(decryptRecoveryArchive(swapped, password));
});
