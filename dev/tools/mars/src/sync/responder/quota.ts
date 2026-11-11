// Capacity accounting of the Sync responder. A checkpoint reserves whole 64 KiB slots in two
// budgets, one for all collections and one for its collection, before it stores any content.
// See `reserve` for the ledger design that works without a mutable counter.
import { encode, fail, LIMITS, reverseSequence } from '../protocol.ts';
import type { Kv, Permit } from './types.ts';
import { NS, parse, clock, b64json, first } from './store.ts';

/** Bytes that one quota slot reserves. A checkpoint reserves whole slots. */
export const UNIT = 64 * 1024;
const COLLECTION_SLOTS = LIMITS.collectionBytes / UNIT;
/** Slots for all collections together. The store holds 1 GiB; the rest stays free for snapshots and metadata. */
export const GLOBAL_SLOTS = (900 * 1024 * 1024) / UNIT;
/**
 * Reserves capacity for one checkpoint in the global budget and in the collection budget, and
 * returns the permit keys in a stable order.
 *
 * Each quota slot is a small immutable generation ledger, not a mutable head. A claimant competes
 * for the next generation key with `ifAbsent`, then reads back the winner. A release appends a
 * free generation, so a delayed duplicate release cannot free the slot of a later claimant.
 * Keys sort newest first, so a scan does not depend on timestamps.
 *
 * A retry with the same `operation` finds its permits through the reference records and takes
 * only the missing ones. A request that runs out of KV operations therefore makes progress.
 */
export async function reserve(
    kv: Kv,
    collection: string,
    operation: string,
    bytes: number,
    checkpoint = '',
): Promise<string[]> {
    const wanted = Math.ceil(bytes / UNIT);
    const result: string[] = [];
    const referencePrefix = `${NS}reservations/${collection}/${operation}/`;
    const references = await kv.list({ prefix: referencePrefix, limit: 64, valuesIncluded: true });
    const known = references.entries.flatMap((entry) => (entry.value ? [parse<string>(entry.value)] : []));
    for (const [prefix, count] of [
        [`${NS}budget/all/`, GLOBAL_SLOTS],
        [`${NS}budget/${collection}/`, COLLECTION_SLOTS],
    ] as const) {
        const owned: string[] = [];
        for (const key of known.filter((key) => key.startsWith(prefix))) {
            const slot = key.slice(0, key.lastIndexOf('/') + 1),
                head = (await kv.list({ prefix: slot, limit: 1, valuesIncluded: true })).entries[0];
            if (head?.key === key && head.value && parse<Permit>(head.value).operation === operation) {
                if (parse<Permit>(head.value).checkpoint !== checkpoint) return fail('operation-reused', 409);
                owned.push(key);
            }
        }
        // Bound collision work. Partial ownership is durable, so Retry makes progress
        // without a second allocation or an invocation beyond the KV operation budget.
        const start = (Number.parseInt(operation.slice(0, 8), 16) + known.length * 41) % count;
        for (let offset = 0; offset < 24 && owned.length < wanted; offset++) {
            const slot = `${prefix}${String((start + offset) % count).padStart(5, '0')}/`;
            const head = (await kv.list({ prefix: slot, limit: 1, valuesIncluded: true })).entries[0];
            const previous = head?.value ? parse<Permit>(head.value) : null;
            if (previous?.busy && previous.operation !== operation) continue;
            let key: string;
            if (previous?.busy) {
                if (previous.checkpoint !== checkpoint) return fail('operation-reused', 409);
                key = head.key;
            } else {
                const generation = (previous?.generation ?? 0) + 1;
                key = `${slot}${reverseSequence(generation)}`;
                const permit: Permit = {
                    collection,
                    operation,
                    checkpoint,
                    reservedAt: clock(),
                    generation,
                    busy: true,
                };
                await kv.set(key, b64json(permit), { ifAbsent: true });
                const entry = await kv.getEntry(key);
                if (entry.expiresAt !== null) return fail('persistent-permits-unavailable', 503);
                if (!entry.value || parse<Permit>(entry.value).operation !== operation) continue;
            }
            if (owned.includes(key)) continue;
            await first(kv, referencePrefix + encode(new TextEncoder().encode(key)), b64json(key), null);
            owned.push(key);
        }
        if (owned.length !== wanted) return fail('sync-storage-full', 507);
        // A retry finds its slots through the reference listing, in another order than the first
        // attempt acquired them. Sort, so both attempts store the identical checkpoint record.
        result.push(...owned.sort());
    }
    return result;
}
/**
 * Frees up to `maximum` permits of `operation`, global and collection permits in turn. A permit
 * that another operation holds, or that is already free, only loses its reference record.
 */
export async function release(kv: Kv, slots: string[], operation: string, maximum = 8, collection?: string) {
    const global = slots.filter((key) => key.startsWith(`${NS}budget/all/`)),
        local = slots.filter((key) => !key.startsWith(`${NS}budget/all/`));
    const interleaved = Array.from({ length: Math.max(global.length, local.length) }, (_, i) => [global[i], local[i]])
        .flat()
        .filter((key): key is string => !!key);
    for (const key of interleaved.slice(0, maximum)) {
        const slot = key.slice(0, key.lastIndexOf('/') + 1),
            head = (await kv.list({ prefix: slot, limit: 1, valuesIncluded: true })).entries[0];
        const permit = head?.value ? parse<Permit>(head.value) : null;
        if (head?.key !== key || !permit?.busy || permit.operation !== operation) {
            const previousCollection = collection ?? (permit?.operation === operation ? permit.collection : undefined);
            if (previousCollection)
                await kv.delete(
                    `${NS}reservations/${previousCollection}/${operation}/${encode(new TextEncoder().encode(key))}`,
                );
            continue;
        }
        const generation = permit.generation + 1,
            next = `${slot}${reverseSequence(generation)}`;
        await kv.set(next, b64json({ ...permit, busy: false, generation }), { ifAbsent: true });
        await kv.delete(`${NS}reservations/${permit.collection}/${operation}/${encode(new TextEncoder().encode(key))}`);
        // Older generations can be pruned safely: no lower generation becomes current.
        const old = await kv.list({ prefix: slot, after: next, limit: 3, valuesIncluded: false });
        for (const entry of old.entries.slice(1)) await kv.delete(entry.key);
    }
}
