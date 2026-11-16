// Complete client-side directory reads. Service metadata is unsigned and untrusted: each page,
// cursor, and the combined item count must pass validation before a caller sees any hierarchy.
import { canonical, LIMITS, SyncError, validId, type SyncDirectory } from './protocol.ts';

/** Reads and validates every directory page. Accepts an older, unpaged response and an optional first index page. */
export async function readDirectory(
    collection: string,
    request: (after?: string) => Promise<unknown>,
    first?: unknown,
): Promise<SyncDirectory> {
    const prefix = `sync/v1/c/${collection}/documents/`;
    const documents: SyncDirectory['documents'] = [];
    const ids = new Set<string>();
    let after: string | undefined;
    let root: SyncDirectory['root'];
    let page: any = first ?? (await request());
    do {
        if (
            !Array.isArray(page?.documents) ||
            page.documents.length > LIMITS.documents ||
            ((page.complete !== undefined || page.cursor !== undefined) &&
                page.documents.length > LIMITS.directoryPageSize) ||
            documents.length + page.documents.length > LIMITS.documents ||
            (page.root !== undefined &&
                (!validId(page.root?.document) || !['doc', 'folder'].includes(page.root.kind))) ||
            (after && canonical(root ?? null) !== canonical(page.root ?? null))
        )
            throw new SyncError('invalid-directory', 503);
        root = page.root;
        for (const item of page.documents) {
            if (
                !validId(item?.id) ||
                !['doc', 'folder'].includes(item.kind) ||
                !validId(item.creator) ||
                typeof item.deleted !== 'boolean' ||
                ids.has(item.id)
            )
                throw new SyncError('invalid-directory', 503);
            ids.add(item.id);
            documents.push(item);
        }
        // An old responder returns no pagination fields. Its complete bounded directory remains readable.
        if (page.complete === undefined && page.cursor === undefined) {
            if (after) throw new SyncError('invalid-directory', 503);
            break;
        }
        if (page.complete === true && page.cursor === null) break;
        if (
            page.complete !== false ||
            !page.documents.length ||
            typeof page.cursor !== 'string' ||
            !page.cursor.startsWith(prefix) ||
            !validId(page.cursor.slice(prefix.length)) ||
            (after && page.cursor <= after)
        )
            throw new SyncError('invalid-directory', 503);
        after = page.cursor;
        page = await request(after);
    } while (true);
    return { documents, ...(root ? { root } : {}) };
}
