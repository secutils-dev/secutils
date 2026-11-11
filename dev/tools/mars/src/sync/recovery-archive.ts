// Binary, authenticated chunks avoid the JSON/base64 expansion of large backups.
// The entire archive is authenticated before the caller can write restored state.
import { encode, decode, fail, utf8 } from './protocol.ts';
import { recoveryKey } from './crypto.ts';

const MAGIC = utf8('MARSREC2');
export const RECOVERY_CHUNK_BYTES = 4 * 1024 * 1024;
export const MAX_RECOVERY_PLAINTEXT_BYTES = 512 * 1024 * 1024;
export const MAX_RECOVERY_ARCHIVE_BYTES = MAX_RECOVERY_PLAINTEXT_BYTES + 65536;
interface Header { v: 2; kind: 'mars-encrypted-recovery'; salt: string; nonce: string; chunkBytes: number; bytes: number; chunks: number }
function iv(prefix: Uint8Array<ArrayBuffer>, index: number) {
    const value = new Uint8Array(12); value.set(prefix); new DataView(value.buffer).setUint32(8, index); return value;
}
const aad = (header: Header, index: number) => utf8(JSON.stringify({ purpose: 'MARS encrypted recovery v2', header, index }));
export async function encryptRecoveryArchive(value: unknown, password: string, chunkBytes = RECOVERY_CHUNK_BYTES) {
    if (password.length < 12) return fail('recovery-password-too-short');
    if (!Number.isSafeInteger(chunkBytes) || chunkBytes < 65536 || chunkBytes > RECOVERY_CHUNK_BYTES) return fail('invalid-recovery-chunk-size');
    const bytes = utf8(JSON.stringify(value));
    if (!bytes.length || bytes.length > MAX_RECOVERY_PLAINTEXT_BYTES) return fail('recovery-package-too-large');
    const salt = crypto.getRandomValues(new Uint8Array(16)), nonce = crypto.getRandomValues(new Uint8Array(8));
    const header: Header = { v: 2, kind: 'mars-encrypted-recovery', salt: encode(salt), nonce: encode(nonce), chunkBytes, bytes: bytes.length, chunks: Math.ceil(bytes.length / chunkBytes) };
    const text = utf8(JSON.stringify(header)), prefix = new Uint8Array(12); prefix.set(MAGIC);
    new DataView(prefix.buffer).setUint32(8, text.length, true);
    const key = await recoveryKey(password, salt), parts: BlobPart[] = [prefix, text];
    for (let index = 0; index < header.chunks; index++) parts.push(await crypto.subtle.encrypt({ name: 'AES-GCM', iv: iv(nonce, index), additionalData: aad(header, index) }, key,
        bytes.subarray(index * chunkBytes, Math.min(bytes.length, (index + 1) * chunkBytes))));
    return new Blob(parts, { type: 'application/octet-stream' });
}
export async function isRecoveryArchive(file: Blob) {
    const prefix = new Uint8Array(await file.slice(0, 8).arrayBuffer());
    return prefix.length === MAGIC.length && prefix.every((value, index) => value === MAGIC[index]);
}
export async function decryptRecoveryArchive<T>(file: Blob, password: string): Promise<T> {
    if (file.size < 12 || file.size > MAX_RECOVERY_ARCHIVE_BYTES || !await isRecoveryArchive(file)) return fail('invalid-recovery-archive');
    const prefix = await file.slice(0, 12).arrayBuffer(), length = new DataView(prefix).getUint32(8, true);
    if (!length || length > 1024) return fail('invalid-recovery-archive');
    const header: Header = JSON.parse(await file.slice(12, 12 + length).text());
    if (!header || header.v !== 2 || header.kind !== 'mars-encrypted-recovery'
        || Object.keys(header).sort().join(',') !== 'bytes,chunkBytes,chunks,kind,nonce,salt,v'
        || !Number.isSafeInteger(header.bytes) || header.bytes < 1 || header.bytes > MAX_RECOVERY_PLAINTEXT_BYTES
        || !Number.isSafeInteger(header.chunkBytes) || header.chunkBytes < 65536 || header.chunkBytes > RECOVERY_CHUNK_BYTES
        || header.chunks !== Math.ceil(header.bytes / header.chunkBytes)
        || file.size !== 12 + length + header.bytes + header.chunks * 16) return fail('invalid-recovery-archive');
    const salt = decode(header.salt), nonce = decode(header.nonce);
    if (salt.length !== 16 || nonce.length !== 8) return fail('invalid-recovery-archive');
    const key = await recoveryKey(password, salt), decoder = new TextDecoder('utf-8', { fatal: true }), parts: string[] = [];
    let offset = 12 + length;
    for (let index = 0; index < header.chunks; index++) {
        const size = Math.min(header.chunkBytes, header.bytes - index * header.chunkBytes) + 16;
        const ciphertext = await file.slice(offset, offset + size).arrayBuffer(); offset += size;
        const plaintext = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: iv(nonce, index), additionalData: aad(header, index) }, key, ciphertext);
        parts.push(decoder.decode(plaintext, { stream: true }));
    }
    parts.push(decoder.decode()); return JSON.parse(parts.join(''));
}
