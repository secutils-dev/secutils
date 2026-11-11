// Binary, authenticated chunks avoid the JSON/base64 expansion of large backups.
// The entire archive is authenticated before the caller can write restored state.
// Layout: the `MARSREC2` marker, a little-endian 32-bit header length, a JSON header, then AES-256-GCM chunks.
// Each chunk ends with its 16-byte tag. The file is untrusted input.
// The key derives from the password, so a wrong password and a changed chunk fail in the same way.
import { encode, decode, fail, utf8 } from './protocol.ts';
import { recoveryKey } from './crypto.ts';

/** File marker: the first eight bytes of each version 2 archive. */
const MAGIC = utf8('MARSREC2');
/** Plaintext bytes of one chunk (4 MiB) and the largest accepted chunk size. The final chunk can be smaller. */
export const RECOVERY_CHUNK_BYTES = 4 * 1024 * 1024;
/** Largest serialized package in bytes (512 MiB). */
export const MAX_RECOVERY_PLAINTEXT_BYTES = 512 * 1024 * 1024;
/**
 * Largest accepted archive file in bytes. The 64 KiB margin holds the prefix, the header, and the tags of 4 MiB chunks.
 */
export const MAX_RECOVERY_ARCHIVE_BYTES = MAX_RECOVERY_PLAINTEXT_BYTES + 65536;
/** Archive header. Each chunk authenticates the complete header, so a changed field fails decryption. */
interface Header {
    v: 2;
    kind: 'mars-encrypted-recovery';
    salt: string;
    nonce: string;
    chunkBytes: number;
    bytes: number;
    chunks: number;
}
/** Nonce of one chunk: the 8-byte random prefix, then the big-endian 32-bit chunk index. */
function iv(prefix: Uint8Array<ArrayBuffer>, index: number) {
    const value = new Uint8Array(12);
    value.set(prefix);
    new DataView(value.buffer).setUint32(8, index);
    return value;
}
/**
 * Associated data of one chunk: the format purpose, the complete header, and the chunk index. A chunk therefore fails
 * in another position or in another archive.
 */
const aad = (header: Header, index: number) =>
    utf8(JSON.stringify({ purpose: 'MARS encrypted recovery v2', header, index }));
/**
 * Encrypts a recovery package as a version 2 archive. The serialized package and all chunks stay in memory.
 * @param password At least 12 characters.
 * @param chunkBytes Plaintext bytes of each chunk, from 64 KiB to 4 MiB.
 * @throws `SyncError` `recovery-password-too-short`, `invalid-recovery-chunk-size`, or `recovery-package-too-large` for
 *   an empty package or one above 512 MiB.
 */
export async function encryptRecoveryArchive(value: unknown, password: string, chunkBytes = RECOVERY_CHUNK_BYTES) {
    if (password.length < 12) return fail('recovery-password-too-short');
    if (!Number.isSafeInteger(chunkBytes) || chunkBytes < 65536 || chunkBytes > RECOVERY_CHUNK_BYTES)
        return fail('invalid-recovery-chunk-size');
    const bytes = utf8(JSON.stringify(value));
    if (!bytes.length || bytes.length > MAX_RECOVERY_PLAINTEXT_BYTES) return fail('recovery-package-too-large');
    const salt = crypto.getRandomValues(new Uint8Array(16)),
        nonce = crypto.getRandomValues(new Uint8Array(8));
    const header: Header = {
        v: 2,
        kind: 'mars-encrypted-recovery',
        salt: encode(salt),
        nonce: encode(nonce),
        chunkBytes,
        bytes: bytes.length,
        chunks: Math.ceil(bytes.length / chunkBytes),
    };
    const text = utf8(JSON.stringify(header)),
        prefix = new Uint8Array(12);
    prefix.set(MAGIC);
    new DataView(prefix.buffer).setUint32(8, text.length, true);
    const key = await recoveryKey(password, salt),
        parts: BlobPart[] = [prefix, text];
    for (let index = 0; index < header.chunks; index++)
        parts.push(
            await crypto.subtle.encrypt(
                { name: 'AES-GCM', iv: iv(nonce, index), additionalData: aad(header, index) },
                key,
                bytes.subarray(index * chunkBytes, Math.min(bytes.length, (index + 1) * chunkBytes)),
            ),
        );
    return new Blob(parts, { type: 'application/octet-stream' });
}
/** True when the file starts with the version 2 marker. It reads eight bytes and proves nothing else. */
export async function isRecoveryArchive(file: Blob) {
    const prefix = new Uint8Array(await file.slice(0, 8).arrayBuffer());
    return prefix.length === MAGIC.length && prefix.every((value, index) => value === MAGIC[index]);
}
/**
 * Decrypts a version 2 archive from an untrusted file and returns the parsed package. Before key derivation it checks
 * the marker, the header fields with their exact key set, and the file size that the header implies. Each chunk then
 * authenticates under its index. It returns only after the last chunk, so a caller never receives a partial package.
 * The parsed value is authentic for the password. Its shape is unchecked.
 * @throws `SyncError` `invalid-recovery-archive` for a structural fault. A wrong password or a changed byte rejects
 *   with a Web Crypto error. Invalid JSON or UTF-8 throws a `SyntaxError` or a `TypeError`.
 */
export async function decryptRecoveryArchive<T>(file: Blob, password: string): Promise<T> {
    if (file.size < 12 || file.size > MAX_RECOVERY_ARCHIVE_BYTES || !(await isRecoveryArchive(file)))
        return fail('invalid-recovery-archive');
    const prefix = await file.slice(0, 12).arrayBuffer(),
        length = new DataView(prefix).getUint32(8, true);
    // The header is short JSON. A longer length is not an archive of this format.
    if (!length || length > 1024) return fail('invalid-recovery-archive');
    const header: Header = JSON.parse(await file.slice(12, 12 + length).text());
    if (
        !header ||
        header.v !== 2 ||
        header.kind !== 'mars-encrypted-recovery' ||
        Object.keys(header).sort().join(',') !== 'bytes,chunkBytes,chunks,kind,nonce,salt,v' ||
        !Number.isSafeInteger(header.bytes) ||
        header.bytes < 1 ||
        header.bytes > MAX_RECOVERY_PLAINTEXT_BYTES ||
        !Number.isSafeInteger(header.chunkBytes) ||
        header.chunkBytes < 65536 ||
        header.chunkBytes > RECOVERY_CHUNK_BYTES ||
        header.chunks !== Math.ceil(header.bytes / header.chunkBytes) ||
        file.size !== 12 + length + header.bytes + header.chunks * 16
    )
        return fail('invalid-recovery-archive');
    const salt = decode(header.salt),
        nonce = decode(header.nonce);
    if (salt.length !== 16 || nonce.length !== 8) return fail('invalid-recovery-archive');
    const key = await recoveryKey(password, salt),
        decoder = new TextDecoder('utf-8', { fatal: true }),
        parts: string[] = [];
    let offset = 12 + length;
    for (let index = 0; index < header.chunks; index++) {
        const size = Math.min(header.chunkBytes, header.bytes - index * header.chunkBytes) + 16;
        const ciphertext = await file.slice(offset, offset + size).arrayBuffer();
        offset += size;
        const plaintext = await crypto.subtle.decrypt(
            { name: 'AES-GCM', iv: iv(nonce, index), additionalData: aad(header, index) },
            key,
            ciphertext,
        );
        parts.push(decoder.decode(plaintext, { stream: true }));
    }
    parts.push(decoder.decode());
    return JSON.parse(parts.join(''));
}
