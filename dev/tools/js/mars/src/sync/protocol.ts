// Versioned, permission-separated messages. No document body or credential belongs in a URL query.
export const VERSION = 1;
export const DAY = 86400;
// collections is the per-IP hourly creation limit; documents counts selected documents and folders.
export const LIMITS = Object.freeze({ collections: 50, documents: 100, members: 10, liveGrants: 50,
    collectionBytes: 100 * 1024 * 1024, updateBytes: 1024 * 1024, inactiveSeconds: 30 * DAY,
    invitationSeconds: 600, liveSeconds: 7 * DAY, maxLiveSeconds: 30 * DAY, pageSize: 32 });
export type Permission = 'view' | 'comment' | 'edit' | 'owner';
export type Stream = 'body' | 'review' | 'tree';
export interface Grant {
    v: 1; collection: string; member: string; participant: string; permission: Permission;
    document: string | null; publicKey: string; boxKey: string; tokenHash: string; expires: number;
}
export interface Invitation {
    v: 1; id: string; collection: string; document: string | null; mode: 'pair' | 'live';
    permission: 'view' | 'comment' | 'edit'; publicKey: string; participant: string | null; expires: number;
}
export interface SignedInvitation { invitation: Invitation; signature: string }
export interface SignedGrant { grant: Grant; signature: string; issuer?: SignedInvitation }
export interface CollectionRoot { document: string; kind: 'doc' | 'folder' }
export interface SyncDirectory { documents: { id: string; kind: 'doc' | 'folder'; creator: string; deleted: boolean }[]; root?: CollectionRoot }
export function checkpointKey(value: string) {
    const parts = value.split('/'), suffix = parts[8]?.split('.');
    if (parts.length !== 9 || parts[0] !== 'sync' || parts[1] !== 'v1' || parts[2] !== 'c' || parts[4] !== 'updates'
        || ![parts[3], parts[5], parts[6]].every(validId) || !['body', 'review', 'tree'].includes(parts[7])
        || suffix?.length !== 2 || !/^\d{16}$/.test(suffix[0]) || !validId(suffix[1])) return null;
    const sequence = Number.MAX_SAFE_INTEGER - Number(suffix[0]);
    return Number.isSafeInteger(sequence) && sequence > 0 ? { collection: parts[3], document: parts[5], member: parts[6], stream: parts[7] as Stream, sequence } : null;
}
export interface Checkpoint {
    v: 1; collection: string; document: string; member: string; stream: Stream;
    epoch: number; sequence: number; operation: string; nonce: string; ciphertext: string;
}
export interface SignedCheckpoint { checkpoint: Checkpoint; signature: string }
export function checkpointReceipt(envelope: SignedCheckpoint, ciphertextHash: string) {
    const { ciphertext: _ciphertext, ...checkpoint } = envelope.checkpoint;
    return { kind: 'mars-sync-checkpoint-receipt-v1', checkpoint, signature: envelope.signature, ciphertextHash };
}
export interface WrappedKey { v: 1; collection: string; document: string; epoch: number; member: string; sender: string; nonce: string; ciphertext: string }
export interface SyncBinding {
    collection: string; document: string; member: string; permission: Permission;
    kind: 'sync' | 'live'; epoch: number; root?: string; participant?: string; ownerParticipant?: string; liveShared?: boolean;
}
export class SyncError extends Error {
    code: string;
    status: number;
    constructor(code: string, status = 400) { super(code); this.name = 'SyncError'; this.code = code; this.status = status; }
}
export const fail = (code: string, status = 400): never => { throw new SyncError(code, status); };
export const validId = (value: unknown): value is string => typeof value === 'string' && /^[a-f0-9]{8}-[a-f0-9]{4}-[1-8][a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(value);
export const validHash = (value: unknown): value is string => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
export const validBase64 = (value: unknown, max: number): value is string => typeof value === 'string' && value.length <= max && value.length % 4 !== 1 && /^[A-Za-z0-9_-]+$/.test(value);
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const keys = (value: Record<string, unknown>, allowed: string[]) => Object.keys(value).length === allowed.length && allowed.every((key) => Object.hasOwn(value, key));
export function validGrant(value: unknown): value is SignedGrant {
    if (!record(value) || !keys(value, value.issuer === undefined ? ['grant', 'signature'] : ['grant', 'signature', 'issuer'])
        || !validBase64(value.signature, 86) || (value.issuer !== undefined && !validInvitation(value.issuer))) return false;
    const g = value.grant;
    return record(g) && keys(g, ['v', 'collection', 'member', 'participant', 'permission', 'document', 'publicKey', 'boxKey', 'tokenHash', 'expires'])
        && g.v === VERSION && [g.collection, g.member, g.participant].every(validId)
        && ['view', 'comment', 'edit', 'owner'].includes(g.permission as string)
        && (g.document === null || validId(g.document)) && validBase64(g.publicKey, 87) && validBase64(g.boxKey, 87)
        && validHash(g.tokenHash) && Number.isSafeInteger(g.expires) && Number(g.expires) >= 0;
}
export function validInvitation(value: unknown): value is SignedInvitation {
    if (!record(value) || !keys(value, ['invitation', 'signature']) || !validBase64(value.signature, 86)) return false;
    const p = value.invitation;
    return record(p) && keys(p, ['v', 'id', 'collection', 'document', 'mode', 'permission', 'publicKey', 'participant', 'expires'])
        && p.v === 1 && validId(p.id) && validId(p.collection) && ['pair', 'live'].includes(p.mode as string)
        && ['view', 'comment', 'edit'].includes(p.permission as string) && validBase64(p.publicKey, 87)
        && Number.isSafeInteger(p.expires) && Number(p.expires) > 0
        && (p.mode === 'pair' ? p.document === null && validId(p.participant) : validId(p.document) && p.participant === null);
}
export function delegatedGrant(signed: SignedGrant, owner: Grant) {
    const p = signed.issuer?.invitation, g = signed.grant;
    const level = { view: 0, comment: 1, edit: 2, owner: 3 };
    return !!p && p.mode === 'live' && p.collection === owner.collection && g.collection === owner.collection
        && g.document === p.document && g.permission !== 'owner' && level[g.permission] <= level[p.permission]
        && g.expires > 0 && g.expires <= p.expires && g.participant === g.member
        && g.participant !== owner.participant && g.member !== owner.member;
}
export function validWrappedKey(value: unknown): value is WrappedKey {
    return record(value) && keys(value, ['v', 'collection', 'document', 'epoch', 'member', 'sender', 'nonce', 'ciphertext'])
        && value.v === 1 && [value.collection, value.document, value.member].every(validId)
        && Number.isSafeInteger(value.epoch) && Number(value.epoch) > 0 && validBase64(value.sender, 87)
        && validBase64(value.nonce, 16) && value.nonce.length === 16 && validBase64(value.ciphertext, 64);
}
export function validCheckpoint(value: unknown): value is SignedCheckpoint {
    if (!record(value) || !keys(value, ['checkpoint', 'signature']) || !validBase64(value.signature, 86)) return false;
    const c = value.checkpoint;
    return record(c) && keys(c, ['v', 'collection', 'document', 'member', 'stream', 'epoch', 'sequence', 'operation', 'nonce', 'ciphertext'])
        && c.v === VERSION && [c.collection, c.document, c.member, c.operation].every(validId)
        && ['body', 'review', 'tree'].includes(c.stream as string) && Number.isSafeInteger(c.epoch) && Number(c.epoch) > 0
        && Number.isSafeInteger(c.sequence) && Number(c.sequence) > 0 && validBase64(c.nonce, 16) && c.nonce.length === 16
        // The 1 MiB limit covers the serialized envelope, not 1 MiB of plaintext
        // plus base64/JSON overhead. Leave space for signatures and storage metadata.
        && validBase64(c.ciphertext, LIMITS.updateBytes - 16384);
}
export function permitted(grant: Grant, document: string, stream: Stream) {
    if (grant.document !== null && grant.document !== document) return false;
    return stream === 'review' ? grant.permission !== 'view'
        : stream === 'tree' ? grant.document === null && ['edit', 'owner'].includes(grant.permission)
            : ['edit', 'owner'].includes(grant.permission);
}

// Exact canonical JSON, shared by Web Crypto and the responder's P-256 verifier.
// Integers only in protocol metadata. Object property order cannot change a signature.
export function canonical(value: unknown): string {
    if (value === null || typeof value === 'boolean' || typeof value === 'string') return JSON.stringify(value);
    if (typeof value === 'number' && Number.isSafeInteger(value)) return JSON.stringify(value);
    if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`;
    if (record(value)) return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`;
    return fail('invalid-canonical-value');
}
const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';
export function encode(bytes: Uint8Array): string {
    const native = (bytes as Uint8Array & { toBase64?: (options: { alphabet: string; omitPadding: boolean }) => string }).toBase64;
    if (native) return native.call(bytes, { alphabet: 'base64url', omitPadding: true });
    // Flatten bounded pieces instead of retaining one V8 rope node per three bytes.
    // The responder has a 10 MiB heap, including signatures and request JSON.
    const chunks: string[] = [];
    for (let start = 0; start < bytes.length; start += 12288) {
        const out: string[] = [];
        const end = Math.min(bytes.length, start + 12288);
        for (let i = start; i < end; i += 3) {
            const a = bytes[i], b = bytes[i + 1], c = bytes[i + 2];
            out.push(B64[a >> 2], B64[((a & 3) << 4) | ((b ?? 0) >> 4)]);
            if (b !== undefined) out.push(B64[((b & 15) << 2) | ((c ?? 0) >> 6)]);
            if (c !== undefined) out.push(B64[c & 63]);
        }
        chunks.push(out.join(''));
    }
    return chunks.join('');
}
export function decode(value: string): Uint8Array<ArrayBuffer> {
    if (!validBase64(value, 16 * 1024 * 1024)) return fail('invalid-base64');
    const out = new Uint8Array(Math.floor(value.length * 3 / 4));
    for (let i = 0, j = 0; i < value.length; i += 4) {
        const a = B64.indexOf(value[i]), b = B64.indexOf(value[i + 1]), c = B64.indexOf(value[i + 2]), d = B64.indexOf(value[i + 3]);
        out[j++] = (a << 2) | (b >> 4);
        if (c >= 0) out[j++] = ((b & 15) << 4) | (c >> 2);
        if (d >= 0) out[j++] = ((c & 3) << 6) | d;
    }
    if (encode(out) !== value) return fail('noncanonical-base64');
    return out;
}
export const utf8 = (value: string) => new TextEncoder().encode(value);
export const jsonBytes = (value: unknown) => utf8(canonical(value));
export const reverseSequence = (sequence: number) => String(Number.MAX_SAFE_INTEGER - sequence).padStart(16, '0');
