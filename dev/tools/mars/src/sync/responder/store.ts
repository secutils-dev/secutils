// Storage primitives of the Sync responder: record keys, the value encoding, immutable writes
// (`first`), numbered slots (`allocate`), signature verification, and the JSON response.
//
// The store has no transactions. A record is written once with `ifAbsent` and read back, so two
// concurrent writers agree on one winner. Every id in a key passed `validId` before it reaches a
// key builder, so a request cannot address a record outside its own prefix.
import { p256 } from '@noble/curves/nist.js';
import {
    decode,
    encode,
    fail,
    jsonBytes,
    LIMITS,
    reverseSequence,
    validBase64,
    delegatedGrant,
    type SignedCheckpoint,
    type SignedGrant,
} from '../protocol.ts';
import type { Kv, Runtime, Entry } from './types.ts';

/** Key prefix of every Sync record. Snapshot links and test scopes use other prefixes. */
export const NS = 'sync/v1/';
/** The current time in seconds, the unit of every expiry in this module. */
export const clock = () => Math.floor(Date.now() / 1000);
/** Encodes a record as canonical JSON in base64url, the value format of the store. */
export const b64json = (value: unknown) => encode(jsonBytes(value));
export const parse = <T>(value: string): T => JSON.parse(new TextDecoder().decode(decode(value)));
/** Verifies an ECDSA P-256 signature over the canonical JSON of `value`. Malformed input is an invalid signature. */
export const signatureValid = (value: unknown, signature: string, publicKey: string) => {
    try {
        return p256.verify(decode(signature), jsonBytes(value), decode(publicKey), { prehash: true, lowS: false });
    } catch {
        return false;
    }
};
/** Builds every response of this module: JSON, never cached, never recorded in request history. */
export const json = (statusCode: number, value?: unknown) => ({
    statusCode,
    headers: {
        'Content-Type': 'application/json; charset=utf-8',
        'Cache-Control': 'no-store',
        'Referrer-Policy': 'no-referrer',
        'X-Content-Type-Options': 'nosniff',
        'Cross-Origin-Resource-Policy': 'same-origin',
    },
    body: value === undefined ? '' : JSON.stringify(value),
    skipRequest: true,
});
// Record keys. Every id in a key passed `validId` first, so a request cannot leave its prefix.
export const grantKey = (collection: string, member: string) => `${NS}c/${collection}/members/${member}`;
export const revokedKey = (collection: string, member: string) => `${NS}c/${collection}/revoked/${member}`;
export const collectionKey = (collection: string) => `${NS}c/${collection}/owner`;
export const docKey = (collection: string, document: string) => `${NS}c/${collection}/documents/${document}`;
export const deletedKey = (collection: string, document: string) => `${NS}c/${collection}/deleted/${document}`;
export const epochsPrefix = (collection: string, document: string) => `${NS}c/${collection}/epochs/${document}/`;
export const inviteKey = (id: string) => `${NS}invitations/${id}`;
export const revokedInviteKey = (collection: string, id: string) => `${NS}c/${collection}/revoked-invitations/${id}`;
/** Key writes per request. The client sends 25 for each request; each write costs up to six KV operations. */
export const MAX_KEY_WRITES = 25;
/** base64url limits for small sealed values: an encrypted device name and an encrypted Live key update. */
export const MAX_LABEL_CHARS = 1024,
    MAX_LIVE_KEY_CHARS = 512;
/**
 * `touch` renews wrapped device keys only below this number of stream heads, to stay inside the KV operation budget.
 */
export const KEY_RENEWAL_HEAD_LIMIT = 100;
export const exactKeys = (value: unknown, keys: string[]): value is Record<string, any> =>
    !!value &&
    typeof value === 'object' &&
    !Array.isArray(value) &&
    Object.keys(value).sort().join(',') === keys.join(',');
/** An AES-GCM envelope as `sealJson` produces it: a 12-byte nonce and a bounded ciphertext. */
export const validSealed = (value: unknown, maxChars: number) =>
    exactKeys(value, ['ciphertext', 'nonce']) &&
    validBase64(value.nonce, 16) &&
    value.nonce.length === 16 &&
    validBase64(value.ciphertext, maxChars) &&
    value.ciphertext.length >= 22;
/** The current key epoch of a document. A document without a rotation is at epoch 1. */
export async function currentEpoch(kv: Kv, collection: string, document: string) {
    const epochs = await kv.list({ prefix: epochsPrefix(collection, document), limit: 1, valuesIncluded: true });
    return epochs.entries[0]?.value ? parse<{ epoch: number }>(epochs.entries[0].value).epoch : 1;
}
export async function liveKeyUpdate(kv: Kv, id: string, collection: string, document: string) {
    return read(kv, `${inviteKey(id)}/keys/${reverseSequence(await currentEpoch(kv, collection, document))}`);
}
/**
 * True when the owner authorized `signed`: directly, or through a Live invitation that the owner
 * signed and whose delegation key signed the grant within the limits of that invitation.
 */
export const signedByOwner = (signed: SignedGrant, owner: SignedGrant) =>
    signed.issuer
        ? delegatedGrant(signed, owner.grant) &&
          signatureValid(signed.issuer.invitation, signed.issuer.signature, owner.grant.publicKey) &&
          signatureValid(signed.grant, signed.signature, signed.issuer.invitation.publicKey)
        : signatureValid(signed.grant, signed.signature, owner.grant.publicKey);
export const updatesPrefix = (collection: string, document = '') =>
    `${NS}c/${collection}/updates/${document}${document ? '/' : ''}`;
export const payloadKey = (checkpoint: string) => checkpoint.replace('/updates/', '/payloads/');
export function takeCiphertext(checkpoint: SignedCheckpoint['checkpoint']) {
    const value = checkpoint.ciphertext;
    checkpoint.ciphertext = '';
    return value;
}
/**
 * Writes an immutable record: creates `key` unless it exists, then reads it back. A different
 * stored value means that another writer holds the key, and the call fails with 409. An equal
 * value makes a retry of the same write succeed.
 *
 * @param ttlSec Seconds until expiry, or `null` for a record that never expires.
 * @param crypto When given, a large value is compared by digest, to keep one copy in the heap.
 */
export async function first(
    kv: Kv,
    key: string,
    value: string,
    ttlSec: number | null = LIMITS.inactiveSeconds,
    crypto?: Runtime['crypto'],
) {
    const digest = crypto && value.length > 65536 ? await crypto.sha256(value) : null;
    await kv.set(key, value, { ifAbsent: true, ...(ttlSec === null ? {} : { ttlSec }) });
    if (digest && crypto) {
        // Drop the encoded write before reading it back. Native KV returns another
        // base64 string; retaining both exceeds the heap near the update-size cap.
        value = '';
        const stored = await kv.get(key);
        if (!stored || (await crypto.sha256(stored)) !== digest) return fail('immutable-record-conflict', 409);
    } else if ((await kv.get(key)) !== value) return fail('immutable-record-conflict', 409);
}
/** Reads and decodes a record that this module wrote, or returns `null` when it is absent or expired. */
export async function read<T>(kv: Kv, key: string): Promise<T | null> {
    const raw = await kv.get(key);
    return raw ? parse<T>(raw) : null;
}
/** Lists every key under `prefix`, and fails with 507 above `ceiling` keys to protect the heap. */
export async function allKeys(kv: Kv, prefix: string, ceiling: number) {
    const entries: Entry[] = [];
    let after: string | undefined;
    do {
        const page = await kv.list({ prefix, after, limit: 1000, valuesIncluded: false });
        entries.push(...page.entries);
        if (entries.length > ceiling) return fail('index-limit', 507);
        if (page.entries.length < 1000) break;
        after = page.cursor ?? undefined;
        if (!after) return fail('invalid-storage-cursor', 503);
    } while (after);
    return entries;
}
/**
 * Takes one of `count` numbered slots under `prefix` for `id` and returns its key. The slot
 * count is the limit: members, Live grants, documents, invitations, or collections for each
 * client address. A second call for the same `id` returns the slot that it already holds.
 */
export async function allocate(
    kv: Kv,
    prefix: string,
    id: string,
    count: number,
    ttlSec = LIMITS.inactiveSeconds,
    limitError = 'membership-or-document-limit',
    limitStatus = 507,
) {
    const encoded = b64json(id);
    for (let i = 0; i < count; i++) {
        const key = `${prefix}${String(i).padStart(3, '0')}`;
        const existing = await kv.get(key);
        if (existing === encoded) return key;
        if (existing) continue;
        await kv.set(key, encoded, { ifAbsent: true, ttlSec });
        if ((await kv.get(key)) === encoded) return key;
    }
    return fail(limitError, limitStatus);
}
/** The response of every action. */
export type SyncResponse = ReturnType<typeof json>;
