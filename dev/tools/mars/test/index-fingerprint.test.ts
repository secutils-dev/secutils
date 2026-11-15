import assert from 'node:assert/strict';
import { test } from 'node:test';
import { inputFingerprint } from '../src/search/fingerprint.ts';
import { indexChunks, MODELS } from '../src/search/models.ts';

test('rename invalidation follows exact model inputs rather than modification timestamps', async () => {
    const chunks = indexChunks('# Heading\n\nThe unchanged document has enough words for a vector.');
    assert.equal(
        await inputFingerprint(MODELS.fast, 'Old name', chunks),
        await inputFingerprint(MODELS.fast, 'New name', chunks),
    );
    assert.notEqual(
        await inputFingerprint(MODELS.best, 'Old name', chunks),
        await inputFingerprint(MODELS.best, 'New name', chunks),
    );
    assert.notEqual(
        await inputFingerprint(MODELS.fast, 'Name', chunks),
        await inputFingerprint(MODELS.fast, 'Name', indexChunks('Different content with enough words for a vector.')),
    );
});

test('position-only changes retain vector identity; review edits and passage order invalidate it', async () => {
    const chunks = indexChunks('A first paragraph with several words.\n\nA second paragraph with several words.');
    const moved = chunks.map((chunk) => ({ ...chunk, start: chunk.start + 20 }));
    assert.equal(
        await inputFingerprint(MODELS.fast, 'Name', chunks),
        await inputFingerprint(MODELS.fast, 'Name', moved),
    );
    const review = indexChunks(
        'A document with several words.\n\n<!-- su-comments\n[{"id":"t","quote":"document","comments":[{"body":"An important review remark here."}]}]\n-->',
    );
    assert.notEqual(
        await inputFingerprint(MODELS.fast, 'Name', review),
        await inputFingerprint(MODELS.fast, 'Name', review.slice(0, 1)),
    );
});
