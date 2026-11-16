import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readDirectory } from '../src/sync/directory.ts';
import { LIMITS, SyncError, type SyncDirectory } from '../src/sync/protocol.ts';

const collection = crypto.randomUUID();
const prefix = `sync/v1/c/${collection}/documents/`;
const item = (): SyncDirectory['documents'][number] => ({
    id: crypto.randomUUID(),
    kind: 'doc',
    creator: crypto.randomUUID(),
    deleted: false,
});

test('directory reads combine every page before returning and accept legacy responses', async () => {
    const documents = Array.from({ length: LIMITS.documents }, item).sort((a, b) => a.id.localeCompare(b.id));
    const root = { document: documents[0].id, kind: 'folder' as const };
    let requests = 0;
    const directory = await readDirectory(collection, async (after) => {
        requests++;
        const start = after ? documents.findIndex((item) => prefix + item.id === after) + 1 : 0;
        const page = documents.slice(start, start + LIMITS.directoryPageSize);
        const complete = start + page.length === documents.length;
        return { documents: page, root, complete, cursor: complete ? null : prefix + page.at(-1)!.id };
    });
    assert.deepEqual(directory, { documents, root });
    assert.equal(requests, 10);
    assert.deepEqual(await readDirectory(collection, async () => ({ documents: documents.slice(0, 2), root })), {
        documents: documents.slice(0, 2),
        root,
    });
});

test('directory reads reject malformed pages, repeated cursors, duplicate identities, and oversized totals', async () => {
    const document = item();
    const first = { documents: [document], complete: false, cursor: prefix + document.id };
    for (const page of [
        { documents: [{}] },
        { documents: [document], root: { document: 'invalid', kind: 'doc' } },
        { documents: [], complete: false, cursor: first.cursor },
        { ...first, cursor: `sync/v1/c/${crypto.randomUUID()}/documents/${document.id}` },
        { ...first, cursor: prefix + 'invalid' },
        { ...first, complete: 'false' },
        { documents: Array.from({ length: LIMITS.directoryPageSize + 1 }, item), complete: true, cursor: null },
        { documents: Array.from({ length: LIMITS.documents + 1 }, item) },
    ])
        await assert.rejects(
            readDirectory(collection, async () => page),
            SyncError,
        );
    await assert.rejects(
        readDirectory(collection, async () => first, first),
        SyncError,
    );
    await assert.rejects(
        readDirectory(collection, async () => ({ documents: [item()] }), first),
        SyncError,
    );
    await assert.rejects(
        readDirectory(collection, async () => ({ documents: [document] }), first),
        SyncError,
    );
    await assert.rejects(
        readDirectory(collection, async () => ({ documents: [item()] }), {
            documents: Array.from({ length: LIMITS.documents }, item),
            complete: false,
            cursor: first.cursor,
        }),
        SyncError,
    );
    await assert.rejects(
        readDirectory(
            collection,
            async () => ({ documents: [item()], root: { document: crypto.randomUUID(), kind: 'folder' } }),
            first,
        ),
        SyncError,
    );
});

test('the combined directory cannot exceed the collection limit through individually valid pages', async () => {
    const documents = Array.from({ length: LIMITS.documents + 1 }, item).sort((a, b) => a.id.localeCompare(b.id));
    let start = 0;
    await assert.rejects(
        readDirectory(collection, async () => {
            const entries = documents.slice(start, start + LIMITS.directoryPageSize);
            start += entries.length;
            const complete = start === documents.length;
            return { documents: entries, complete, cursor: complete ? null : prefix + entries.at(-1)!.id };
        }),
        SyncError,
    );
    assert.equal(start, LIMITS.documents + 1);
});

test('directory read failure never returns a partial hierarchy', async () => {
    const document = item();
    const aborted = new DOMException('Cancelled', 'AbortError');
    await assert.rejects(
        readDirectory(
            collection,
            async () => {
                throw aborted;
            },
            { documents: [document], complete: false, cursor: prefix + document.id },
        ),
        aborted,
    );
});
