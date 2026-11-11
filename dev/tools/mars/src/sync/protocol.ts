// Wire protocol of Sync and Live links: message types, validators, canonical JSON, and base64url.
// The browser client and the server-side responder both compile this file.
// It uses no Web Crypto, DOM, or storage API.
// Validators treat their input as untrusted. They check exact key sets, value formats, and size limits.
// They prove nothing about authorship: signature and delegation checks live in crypto.ts and responder.ts.
// Versioned, permission-separated messages. No document body or credential belongs in a URL query.

/** Protocol version in the `v` field of each wire message. Validators reject any other value. */
export const VERSION = 1;
/** Seconds in one day. */
export const DAY = 86400;
/**
 * Limits that the client and the responder share. Sizes are bytes. Durations are seconds.
 * - `collections`: collections that one client address can create in one hour.
 * - `documents`: selected documents and folders in one collection.
 * - `members`: device grants in one collection.
 * - `liveGrants`: Live grants in one collection. The same number bounds its invitations.
 * - `collectionBytes`: checkpoint storage budget of one collection.
 * - `updateBytes`: one request body, and therefore one serialized checkpoint envelope.
 * - `inactiveSeconds`: lifetime of checkpoints and activity leases without renewal.
 * - `invitationSeconds`: longest lifetime of a pairing invitation.
 * - `maxLiveSeconds`: longest lifetime of a Live invitation or a Live grant. `liveSeconds` is a shorter reference
 *   value.
 * - `pageSize`: checkpoint keys in one index page.
 */
export const LIMITS = Object.freeze({
    collections: 50,
    documents: 100,
    members: 10,
    liveGrants: 50,
    collectionBytes: 100 * 1024 * 1024,
    updateBytes: 1024 * 1024,
    inactiveSeconds: 30 * DAY,
    invitationSeconds: 600,
    liveSeconds: 7 * DAY,
    maxLiveSeconds: 30 * DAY,
    pageSize: 32,
});
/** Access level of a grant, from lowest to highest. `permitted` maps it to writable streams. */
export type Permission = 'view' | 'comment' | 'edit' | 'owner';
/**
 * Independent checkpoint streams of one document. `body` carries the Yjs state of a document. `tree` carries the Yjs
 * state of a folder with its manifest. `review` carries signed review operations as JSON.
 */
export type Stream = 'body' | 'review' | 'tree';
/**
 * Access of one member to a collection. The owner signs it, or a Live invitation key signs it as a delegation.
 * - `member`: ID of this grant. It also names the checkpoint streams of the member.
 * - `participant`: review author identity. Paired devices share the value of the owner. A Live grant uses its member
 *   ID.
 * - `document`: `null` covers the whole collection. An ID restricts the grant to one document.
 * - `publicKey`: raw P-256 ECDSA key in base64url. It verifies the checkpoints and review operations of the member.
 * - `boxKey`: raw P-256 ECDH key in base64url. Wrapped document keys are encrypted for it.
 * - `tokenHash`: SHA-256 (hex) of the bearer token of the member. The grant never contains the token.
 * - `expires`: Unix seconds. 0 means no expiry.
 */
export interface Grant {
    v: 1;
    collection: string;
    member: string;
    participant: string;
    permission: Permission;
    document: string | null;
    publicKey: string;
    boxKey: string;
    tokenHash: string;
    expires: number;
}
/**
 * Owner-signed policy of an invitation. `expires` is Unix seconds.
 * - `pair`: invites a device to the whole collection (`document` is `null`) under the named participant.
 * - `live`: delegates grant signing for one document to the key pair behind `publicKey`. A delegated grant cannot
 *   exceed `permission` or outlive `expires` (`delegatedGrant`).
 */
export interface Invitation {
    v: 1;
    id: string;
    collection: string;
    document: string | null;
    mode: 'pair' | 'live';
    permission: 'view' | 'comment' | 'edit';
    publicKey: string;
    participant: string | null;
    expires: number;
}
/** An invitation with the ECDSA signature of the owner over its canonical JSON. */
export interface SignedInvitation {
    invitation: Invitation;
    signature: string;
}
/**
 * A grant with a signature over its canonical JSON. Without `issuer`, the owner signs. With `issuer`, the invitation
 * key signs the grant and the owner signs the invitation.
 */
export interface SignedGrant {
    grant: Grant;
    signature: string;
    issuer?: SignedInvitation;
}
/** The root item of a collection: one document or the top folder. */
export interface CollectionRoot {
    document: string;
    kind: 'doc' | 'folder';
}
/**
 * The registry of the items in a collection, as the responder returns it, with the optional root. It carries no
 * signature. The transport checks its shape and size only.
 */
export interface SyncDirectory {
    documents: { id: string; kind: 'doc' | 'folder'; creator: string; deleted: boolean }[];
    root?: CollectionRoot;
}
/**
 * Parses a checkpoint index key of this form:
 * `sync/v1/c/<collection>/updates/<document>/<member>/<stream>/<reversed sequence>.<operation>`
 * Keys come from service responses, so the function validates each segment. It returns `null` for any other shape. The
 * result is a download hint. The signed envelope remains the authority for these fields.
 */
export function checkpointKey(value: string) {
    const parts = value.split('/'),
        suffix = parts[8]?.split('.');
    if (
        parts.length !== 9 ||
        parts[0] !== 'sync' ||
        parts[1] !== 'v1' ||
        parts[2] !== 'c' ||
        parts[4] !== 'updates' ||
        ![parts[3], parts[5], parts[6]].every(validId) ||
        !['body', 'review', 'tree'].includes(parts[7]) ||
        suffix?.length !== 2 ||
        !/^\d{16}$/.test(suffix[0]) ||
        !validId(suffix[1])
    )
        return null;
    const sequence = Number.MAX_SAFE_INTEGER - Number(suffix[0]);
    return Number.isSafeInteger(sequence) && sequence > 0
        ? { collection: parts[3], document: parts[5], member: parts[6], stream: parts[7] as Stream, sequence }
        : null;
}
/**
 * One encrypted full-state update of one stream by one member. Each field except `nonce` and `ciphertext` is AES-GCM
 * associated data. The member signs the complete object.
 * - `epoch`: key epoch of the document key that encrypts `ciphertext`.
 * - `sequence`: position in the stream of the author. Receivers ignore a value at or below their cursor.
 * - `operation`: idempotency ID. A retry sends the same ID with the same bytes.
 * - `nonce`: 12-byte AES-GCM nonce. `ciphertext`: ciphertext with its tag. Both are base64url.
 */
export interface Checkpoint {
    v: 1;
    collection: string;
    document: string;
    member: string;
    stream: Stream;
    epoch: number;
    sequence: number;
    operation: string;
    nonce: string;
    ciphertext: string;
}
/** A checkpoint with the ECDSA signature of its author over the canonical JSON, ciphertext included. */
export interface SignedCheckpoint {
    checkpoint: Checkpoint;
    signature: string;
}
/**
 * Builds the acknowledgement transcript of an upload. The responder and the client hash it to the same digest. It binds
 * each signed field, the signature, and the ciphertext hash under a domain-separation tag. The ciphertext itself stays
 * out, so the hash input stays small.
 * @param ciphertextHash SHA-256 (hex) of the ciphertext bytes.
 */
export function checkpointReceipt(envelope: SignedCheckpoint, ciphertextHash: string) {
    const { ciphertext: _ciphertext, ...checkpoint } = envelope.checkpoint;
    return { kind: 'mars-sync-checkpoint-receipt-v1', checkpoint, signature: envelope.signature, ciphertextHash };
}
/**
 * A document key of one epoch, encrypted for one member. `sender` is the raw ECDH public key of the sender. Each field
 * except `nonce` and `ciphertext` is associated data and HKDF info. A wrapped key therefore fails to open for another
 * collection, document, epoch, member, or sender.
 */
export interface WrappedKey {
    v: 1;
    collection: string;
    document: string;
    epoch: number;
    member: string;
    sender: string;
    nonce: string;
    ciphertext: string;
}
/**
 * Local identity of one connected document: its collection, its remote document ID, and the member grant in use. It is
 * client state, not a wire message.
 * - `kind`: `live` for a document that arrived through a Live link, otherwise `sync`.
 * - `epoch`: key epoch that local uploads use.
 * - `root`: remote ID of the scope root.
 * - `participant`, `ownerParticipant`: copies of the participant IDs in the member grant and the owner grant.
 * - `liveShared`: the owner shares this document through a Live link.
 */
export interface SyncBinding {
    collection: string;
    document: string;
    member: string;
    permission: Permission;
    kind: 'sync' | 'live';
    epoch: number;
    root?: string;
    participant?: string;
    ownerParticipant?: string;
    liveShared?: boolean;
}
/**
 * A protocol failure. `code` is a fixed identifier and also the message. `status` follows HTTP semantics. Callers
 * branch on `code` and `status`, never on free text.
 */
export class SyncError extends Error {
    code: string;
    status: number;
    constructor(code: string, status = 400) {
        super(code);
        this.name = 'SyncError';
        this.code = code;
        this.status = status;
    }
}
/** Throws a `SyncError`. The `never` result lets a caller write `return fail(...)` in any function. */
export const fail = (code: string, status = 400): never => {
    throw new SyncError(code, status);
};
/**
 * A downloaded checkpoint that failed local verification: a bad signature, undecryptable or
 * malformed content, or an operation that its author may not perform. The service cannot inspect
 * ciphertext, so any participant with write access can upload one. A receiver skips that one
 * checkpoint and keeps the connection. It is not a service failure.
 */
export class RejectedPacket extends SyncError {
    constructor(cause: unknown) {
        super(cause instanceof SyncError ? cause.code : 'invalid-checkpoint', 422);
        this.name = 'RejectedPacket';
    }
}
/**
 * Accepts a lowercase UUID in canonical form. IDs become KV key segments, so the fixed alphabet also keeps `/` and
 * other separators out of keys.
 */
export const validId = (value: unknown): value is string =>
    typeof value === 'string' &&
    /^[a-f0-9]{8}-[a-f0-9]{4}-[1-8][a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(value);
/** Accepts 64 lowercase hex characters: a SHA-256 digest. */
export const validHash = (value: unknown): value is string => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
/**
 * Accepts non-empty unpadded base64url text of at most `max` characters. It checks the alphabet and the length only.
 * `decode` also rejects non-canonical trailing bits.
 */
export const validBase64 = (value: unknown, max: number): value is string =>
    typeof value === 'string' && value.length <= max && value.length % 4 !== 1 && /^[A-Za-z0-9_-]+$/.test(value);
const record = (value: unknown): value is Record<string, unknown> =>
    !!value && typeof value === 'object' && !Array.isArray(value);
/** True when `value` has exactly the `allowed` own keys. An unknown key fails validation. */
const keys = (value: Record<string, unknown>, allowed: string[]) =>
    Object.keys(value).length === allowed.length && allowed.every((key) => Object.hasOwn(value, key));
/**
 * Shape check of an untrusted signed grant: exact keys, ID and hash formats, bounded key and signature lengths. An
 * `issuer`, when present, must be a valid signed invitation. It verifies no signature and no delegation limit. Use
 * `verifyGrant` in crypto.ts for those.
 */
export function validGrant(value: unknown): value is SignedGrant {
    if (
        !record(value) ||
        !keys(value, value.issuer === undefined ? ['grant', 'signature'] : ['grant', 'signature', 'issuer']) ||
        !validBase64(value.signature, 86) ||
        (value.issuer !== undefined && !validInvitation(value.issuer))
    )
        return false;
    const g = value.grant;
    return (
        record(g) &&
        keys(g, [
            'v',
            'collection',
            'member',
            'participant',
            'permission',
            'document',
            'publicKey',
            'boxKey',
            'tokenHash',
            'expires',
        ]) &&
        g.v === VERSION &&
        [g.collection, g.member, g.participant].every(validId) &&
        ['view', 'comment', 'edit', 'owner'].includes(g.permission as string) &&
        (g.document === null || validId(g.document)) &&
        validBase64(g.publicKey, 87) &&
        validBase64(g.boxKey, 87) &&
        validHash(g.tokenHash) &&
        Number.isSafeInteger(g.expires) &&
        Number(g.expires) >= 0
    );
}
/**
 * Shape check of an untrusted signed invitation. A `pair` invitation has no document and names a participant. A `live`
 * invitation names one document and no participant. It verifies no signature.
 */
export function validInvitation(value: unknown): value is SignedInvitation {
    if (!record(value) || !keys(value, ['invitation', 'signature']) || !validBase64(value.signature, 86)) return false;
    const p = value.invitation;
    return (
        record(p) &&
        keys(p, ['v', 'id', 'collection', 'document', 'mode', 'permission', 'publicKey', 'participant', 'expires']) &&
        p.v === 1 &&
        validId(p.id) &&
        validId(p.collection) &&
        ['pair', 'live'].includes(p.mode as string) &&
        ['view', 'comment', 'edit'].includes(p.permission as string) &&
        validBase64(p.publicKey, 87) &&
        Number.isSafeInteger(p.expires) &&
        Number(p.expires) > 0 &&
        (p.mode === 'pair'
            ? p.document === null && validId(p.participant)
            : validId(p.document) && p.participant === null)
    );
}
/**
 * Checks that a delegated grant stays inside the Live invitation that issued it. The grant holder chooses each grant
 * field, so the function compares them with the owner-signed policy. It requires the same collection and document, a
 * permission at or below that of the invitation and never `owner`, and an expiry inside that of the invitation. The
 * grant must be its own participant, and its member and participant must differ from those of the owner. It therefore
 * cannot claim the review identity of another participant. It verifies no signature. The caller checks the invitation
 * against the owner key and the grant against the invitation key.
 */
export function delegatedGrant(signed: SignedGrant, owner: Grant) {
    const p = signed.issuer?.invitation,
        g = signed.grant;
    const level = { view: 0, comment: 1, edit: 2, owner: 3 };
    return (
        !!p &&
        p.mode === 'live' &&
        p.collection === owner.collection &&
        g.collection === owner.collection &&
        g.document === p.document &&
        g.permission !== 'owner' &&
        level[g.permission] <= level[p.permission] &&
        g.expires > 0 &&
        g.expires <= p.expires &&
        g.participant === g.member &&
        g.participant !== owner.participant &&
        g.member !== owner.member
    );
}
/**
 * Shape check of an untrusted wrapped key: exact keys, a 12-byte nonce, and a ciphertext of at most 64 characters. That
 * length fits a 32-byte key with its tag. It proves nothing about the sender.
 */
export function validWrappedKey(value: unknown): value is WrappedKey {
    return (
        record(value) &&
        keys(value, ['v', 'collection', 'document', 'epoch', 'member', 'sender', 'nonce', 'ciphertext']) &&
        value.v === 1 &&
        [value.collection, value.document, value.member].every(validId) &&
        Number.isSafeInteger(value.epoch) &&
        Number(value.epoch) > 0 &&
        validBase64(value.sender, 87) &&
        validBase64(value.nonce, 16) &&
        value.nonce.length === 16 &&
        validBase64(value.ciphertext, 64)
    );
}
/**
 * Shape check of an untrusted signed checkpoint: exact keys, valid IDs, a known stream, a positive epoch and sequence,
 * a 12-byte nonce, and a bounded ciphertext. The responder applies it to uploads and the client applies it to
 * downloads. It verifies neither the signature nor the permission of the author.
 */
export function validCheckpoint(value: unknown): value is SignedCheckpoint {
    if (!record(value) || !keys(value, ['checkpoint', 'signature']) || !validBase64(value.signature, 86)) return false;
    const c = value.checkpoint;
    return (
        record(c) &&
        keys(c, [
            'v',
            'collection',
            'document',
            'member',
            'stream',
            'epoch',
            'sequence',
            'operation',
            'nonce',
            'ciphertext',
        ]) &&
        c.v === VERSION &&
        [c.collection, c.document, c.member, c.operation].every(validId) &&
        ['body', 'review', 'tree'].includes(c.stream as string) &&
        Number.isSafeInteger(c.epoch) &&
        Number(c.epoch) > 0 &&
        Number.isSafeInteger(c.sequence) &&
        Number(c.sequence) > 0 &&
        validBase64(c.nonce, 16) &&
        c.nonce.length === 16 &&
        // The 1 MiB limit covers the serialized envelope, not 1 MiB of plaintext
        // plus base64/JSON overhead. Leave space for signatures and storage metadata.
        validBase64(c.ciphertext, LIMITS.updateBytes - 16384)
    );
}
/**
 * Decides whether a grant can write to a stream of a document.
 * - `review`: any permission above `view`.
 * - `body`: `edit` or `owner`.
 * - `tree`: `edit` or `owner`, and only with a whole-collection grant. A single-document grant never reaches another
 *   document. The responder applies this rule to uploads. Receivers apply it again to each downloaded checkpoint.
 */
export function permitted(grant: Grant, document: string, stream: Stream) {
    if (grant.document !== null && grant.document !== document) return false;
    return stream === 'review'
        ? grant.permission !== 'view'
        : stream === 'tree'
          ? grant.document === null && ['edit', 'owner'].includes(grant.permission)
          : ['edit', 'owner'].includes(grant.permission);
}

/**
 * Exact canonical JSON, shared by Web Crypto and the responder's P-256 verifier. Integers only in protocol metadata.
 * Object property order cannot change a signature. The output has sorted keys and no whitespace.
 * @throws `SyncError` `invalid-canonical-value` for `undefined`, a fraction, an unsafe integer, or a non-JSON type.
 */
export function canonical(value: unknown): string {
    if (value === null || typeof value === 'boolean' || typeof value === 'string') return JSON.stringify(value);
    if (typeof value === 'number' && Number.isSafeInteger(value)) return JSON.stringify(value);
    if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
    if (record(value))
        return `{${Object.keys(value)
            .sort()
            .map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`)
            .join(',')}}`;
    return fail('invalid-canonical-value');
}
/** The base64url alphabet. */
const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';
/** Encodes bytes as unpadded base64url. It uses the native `toBase64` method when the runtime has one. */
export function encode(bytes: Uint8Array): string {
    const native = (
        bytes as Uint8Array & { toBase64?: (options: { alphabet: string; omitPadding: boolean }) => string }
    ).toBase64;
    if (native) return native.call(bytes, { alphabet: 'base64url', omitPadding: true });
    // Flatten bounded pieces instead of retaining one V8 rope node per three bytes.
    // The responder has a 10 MiB heap, including signatures and request JSON.
    const chunks: string[] = [];
    for (let start = 0; start < bytes.length; start += 12288) {
        const out: string[] = [];
        const end = Math.min(bytes.length, start + 12288);
        for (let i = start; i < end; i += 3) {
            const a = bytes[i],
                b = bytes[i + 1],
                c = bytes[i + 2];
            out.push(B64[a >> 2], B64[((a & 3) << 4) | ((b ?? 0) >> 4)]);
            if (b !== undefined) out.push(B64[((b & 15) << 2) | ((c ?? 0) >> 6)]);
            if (c !== undefined) out.push(B64[c & 63]);
        }
        chunks.push(out.join(''));
    }
    return chunks.join('');
}
/**
 * Decodes unpadded base64url text of at most 16,777,216 characters. It rejects text that does not encode back to
 * itself, so each byte string has exactly one accepted spelling.
 * @throws `SyncError` `invalid-base64` or `noncanonical-base64`.
 */
export function decode(value: string): Uint8Array<ArrayBuffer> {
    if (!validBase64(value, 16 * 1024 * 1024)) return fail('invalid-base64');
    const out = new Uint8Array(Math.floor((value.length * 3) / 4));
    for (let i = 0, j = 0; i < value.length; i += 4) {
        const a = B64.indexOf(value[i]),
            b = B64.indexOf(value[i + 1]),
            c = B64.indexOf(value[i + 2]),
            d = B64.indexOf(value[i + 3]);
        out[j++] = (a << 2) | (b >> 4);
        if (c >= 0) out[j++] = ((b & 15) << 4) | (c >> 2);
        if (d >= 0) out[j++] = ((c & 3) << 6) | d;
    }
    if (encode(out) !== value) return fail('noncanonical-base64');
    return out;
}
export const utf8 = (value: string) => new TextEncoder().encode(value);
/** UTF-8 bytes of the canonical JSON of `value`: the exact input of each signature. */
export const jsonBytes = (value: unknown) => utf8(canonical(value));
/**
 * Formats a sequence for a KV key so that a higher sequence sorts first: `MAX_SAFE_INTEGER - sequence`, zero-padded to
 * 16 digits.
 */
export const reverseSequence = (sequence: number) => String(Number.MAX_SAFE_INTEGER - sequence).padStart(16, '0');
