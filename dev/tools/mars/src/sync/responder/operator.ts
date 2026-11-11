// Operator actions of the Sync responder: capacity telemetry, bounded cleanup of abandoned
// reservations, and the read-only storage audit. The operator secret of the responder authorizes
// them. No response contains document content, credentials, or participant identifiers.
import { encode, fail, validId } from '../protocol.ts';
import { auditStoragePage } from '../storage-audit.ts';
import type { Context, Runtime, SyncRequest, Permit } from './types.ts';
import { json, NS, parse, deletedKey, clock, payloadKey } from './store.ts';
import { UNIT, GLOBAL_SLOTS, release } from './quota.ts';

/**
 * Handles the operator actions: `telemetry`, `maintenance`, and `storage-audit`. The operator secret of the responder
 * authorizes them.
 */
export async function operatorAction(context: Context, runtime: Runtime, request: SyncRequest, action: string) {
    const { kv } = runtime;

    const secret = context.secrets?.MARS_STATS_API_KEY ?? context.params?.secrets?.MARS_STATS_API_KEY;
    const token = context.headers?.authorization?.replace(/^Bearer /, '');
    if (!secret) return fail('sync-telemetry-not-configured', 503);
    if (
        !token ||
        (await runtime.crypto.sha256(encode(new TextEncoder().encode(token)))) !==
            (await runtime.crypto.sha256(encode(new TextEncoder().encode(secret))))
    )
        return fail('forbidden', 403);
    if (action === 'storage-audit') return json(200, await auditStoragePage(request, runtime));
    const prefix = `${NS}budget/all/`;
    if (action === 'telemetry') {
        let after: string | undefined,
            currentSlot = '',
            reservedSlots = 0;
        const collections = new Set<string>();
        do {
            const page = await kv.list({ prefix, after, limit: 1000, valuesIncluded: true });
            for (const entry of page.entries) {
                const slot = entry.key.slice(0, entry.key.lastIndexOf('/'));
                if (slot === currentSlot) continue;
                currentSlot = slot;
                const permit = entry.value ? parse<Permit>(entry.value) : null;
                if (permit?.busy) {
                    reservedSlots++;
                    collections.add(permit.collection);
                }
            }
            if (page.entries.length < 1000) break;
            after = page.cursor ?? undefined;
            if (!after) return fail('invalid-storage-cursor', 503);
        } while (after);
        // Reserved capacity is not actual stored bytes or an active-reader count.
        // No document, collection, participant, or credential identifiers leave this endpoint.
        return json(200, {
            reservedBytes: reservedSlots * UNIT,
            maximumReservedBytes: GLOBAL_SLOTS * UNIT,
            collectionsWithReservations: collections.size,
            reservedSlots,
            unitBytes: UNIT,
        });
    }
    if (
        request.after &&
        (typeof request.after !== 'string' || !request.after.startsWith(prefix) || request.after.length > 256)
    )
        return fail('invalid-maintenance-cursor');
    const page = await kv.list({ prefix, after: request.after || undefined, limit: 8, valuesIncluded: true });
    let reclaimedOperations = 0,
        currentSlot = '',
        cursor = page.cursor;
    for (const entry of page.entries) {
        const slot = entry.key.slice(0, entry.key.lastIndexOf('/') + 1);
        if (slot === currentSlot) continue;
        currentSlot = slot;
        const head = (await kv.list({ prefix: slot, limit: 1, valuesIncluded: true })).entries[0];
        const permit = head?.value ? parse<Permit>(head.value) : null;
        // Abandoned reservations receive a two-day grace period. Ordinary request
        // execution is bounded to seconds. No in-flight upload is reclaimed here.
        if (!permit?.busy || !permit.checkpoint) continue;
        const record = await kv.get(permit.checkpoint);
        const document = permit.checkpoint.split('/')[5];
        const deleted = validId(document) && !!(await kv.get(deletedKey(permit.collection, document)));
        if (record) {
            const prefix = permit.checkpoint.slice(0, permit.checkpoint.lastIndexOf('/') + 1);
            const newest = await kv.list({ prefix, limit: 2, valuesIncluded: false });
            if (!deleted && (newest.entries.length < 2 || newest.entries.some(({ key }) => key === permit.checkpoint)))
                continue;
            await kv.delete(permit.checkpoint);
        } else if (
            !deleted &&
            permit.reservedAt > clock() - 2 * 86400 &&
            !(await kv.get(`${NS}c/${permit.collection}/receipts/${permit.operation}`))
        )
            continue;
        await kv.delete(payloadKey(permit.checkpoint));
        const refs = await kv.list({
            prefix: `${NS}reservations/${permit.collection}/${permit.operation}/`,
            limit: 64,
            valuesIncluded: true,
        });
        await release(
            kv,
            refs.entries.flatMap((ref) => (ref.value ? [parse<string>(ref.value)] : [])),
            permit.operation,
            8,
            permit.collection,
        );
        reclaimedOperations++;
        cursor = entry.key;
        break; // One potentially large reservation per bounded request.
    }
    return json(200, {
        reclaimedOperations,
        cursor,
        complete: !reclaimedOperations && page.entries.length < 8,
    });
}
