// Wire formats for sharing (pure; runs in browsers and Node >= 22 for tests).
//
// URL state: see dev/tools/shared/guides/url-state.md -> "URL state encoding":
//   base64url( u32le(uncompressed length) || deflate-raw(UTF-8 text) )
// Full links: `#name=<name>&md=<state>&o=<originId>&w=<readingWidth>` for one document
// (compatible with Markdown Preview, which ignores `o` and `w`), or `#b=<state of a bundle JSON>`
// for a folder.
// Short links: `#s=<secret>`; the responder stores base64url(IV || AES-GCM(state bytes of the
// bundle JSON)) under `id`. Both `id` and the AES key are SHA-256 hashes of the secret, so
// the key never leaves the fragment (same scheme as Markdown Preview, different plaintext).
//
// Trust: a link can come from anyone, and a short-link blob comes from the service. Each decoder
// bounds the inflated size (`MAX_STATE_BYTES`), `parseBundle` checks the bundle shape, and
// `decodeState`, `parseBundle`, and `openSealed` return null for bad input. They do not throw.
// Document names and text stay untrusted content after a successful decode.

import { validWidth, type ReadingWidth } from '../export/settings.ts';

const enc = new TextEncoder();
const dec = new TextDecoder();

/** Encodes bytes as base64url without padding. */
export const toBase64Url = (bytes: Uint8Array) => {
    let s = '';
    for (const b of bytes) s += String.fromCharCode(b);
    return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
};

/** Decodes base64url, with or without padding. Throws on a character outside the alphabet. */
export const fromBase64Url = (str: string) => {
    const b64 = str.replace(/-/g, '+').replace(/_/g, '/');
    const bin = atob(b64 + '==='.slice(0, (4 - (b64.length % 4)) % 4));
    return Uint8Array.from(bin, (c) => c.charCodeAt(0));
};

// Runs `bytes` through a compression or decompression stream and collects the complete output.
async function pipe(bytes: Uint8Array, stream: CompressionStream | DecompressionStream) {
    return new Uint8Array(await new Response(new Blob([bytes as BlobPart]).stream().pipeThrough(stream)).arrayBuffer());
}

/** Encodes text as state bytes: `u32le(UTF-8 length) || deflate-raw(UTF-8 text)`. */
export async function encodeStateBytes(text: string) {
    const bytes = enc.encode(text);
    const deflated = await pipe(bytes, new CompressionStream('deflate-raw'));
    const out = new Uint8Array(4 + deflated.length);
    new DataView(out.buffer).setUint32(0, bytes.length, true);
    out.set(deflated, 4);
    return out;
}

/**
 * The largest text, in UTF-8 bytes (64 MiB), that a link, a snapshot, or a bundle can expand to.
 * DEFLATE reaches about 1000:1, so an unbounded read lets a small link fill memory and storage.
 */
export const MAX_STATE_BYTES = 64 * 1024 * 1024;

// Inflates raw DEFLATE data in chunks and stops as soon as the output exceeds `limit` bytes.
// The declared length cannot be trusted, so this count is the effective bound.
// Throws for oversized output and for corrupt data.
async function inflate(bytes: Uint8Array, limit: number) {
    const reader = new Blob([bytes as BlobPart])
        .stream()
        .pipeThrough(new DecompressionStream('deflate-raw'))
        .getReader();
    const chunks: Uint8Array[] = [];
    let total = 0;
    for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        total += value.length;
        if (total > limit) {
            await reader.cancel();
            throw new Error('State is too large');
        }
        chunks.push(value);
    }
    const out = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) {
        out.set(chunk, offset);
        offset += chunk.length;
    }
    return out;
}

/**
 * Decodes state bytes to text. `limit` is the largest accepted output in bytes.
 * Throws when the input is shorter than the prefix, and when the declared or the actual size
 * exceeds `limit`. Also throws when the compressed data is corrupt. Invalid UTF-8 becomes U+FFFD.
 */
export async function decodeStateBytes(bytes: Uint8Array, limit = MAX_STATE_BYTES) {
    if (bytes.length < 4) throw new Error('Truncated state');
    // The prefix is the declared size. Reject an oversized claim early, and bound the actual output too.
    if (new DataView(bytes.buffer, bytes.byteOffset, 4).getUint32(0, true) > limit)
        throw new Error('State is too large');
    return dec.decode(await inflate(bytes.subarray(4), limit));
}

/** Encodes text as a URL state string (base64url of the state bytes). */
export const encodeState = async (text: string) => toBase64Url(await encodeStateBytes(text));
/** Decodes a URL state string. Returns null for any malformed, truncated, or oversized input. */
export const decodeState = async (str: string) => {
    try {
        return await decodeStateBytes(fromBase64Url(str));
    } catch {
        return null;
    }
};

/** One document of a bundle. */
export interface SharedDoc {
    // Folder names from the shared root down to the document's parent.
    path: string[];
    name: string;
    originId: string;
    text: string;
    readingWidth?: ReadingWidth;
}
/**
 * The plaintext of a folder link and of every short link: one document, or a folder with its
 * documents. Accept one from outside only through `parseBundle`.
 */
export interface Bundle {
    // Format version. A parser rejects every other value.
    v: 1;
    kind: 'doc' | 'folder';
    name: string;
    // The shared folder's originId, so a re-shared folder merges into the same copy.
    originId?: string;
    docs: SharedDoc[];
    // The sender shared from a temporary session. The recipient opens the link in one too.
    temporary?: true;
}

/**
 * Validates untrusted bundle JSON. Returns the bundle, or null when the JSON does not parse
 * or a required field has the wrong type.
 * It checks types only: `v`, `kind`, `name`, and for each document `text`, `name`, `originId`,
 * and `path`. It removes an invalid `readingWidth` and a `temporary` value other than `true`.
 * It does not check the optional folder `originId`, does not limit the number of documents,
 * and does not remove unknown fields.
 */
export function parseBundle(json: string): Bundle | null {
    try {
        const b = JSON.parse(json);
        const docOk = (d: SharedDoc) =>
            d &&
            typeof d.text === 'string' &&
            typeof d.name === 'string' &&
            typeof d.originId === 'string' &&
            Array.isArray(d.path) &&
            d.path.every((p) => typeof p === 'string');
        if (
            b?.v !== 1 ||
            !['doc', 'folder'].includes(b.kind) ||
            typeof b.name !== 'string' ||
            !Array.isArray(b.docs) ||
            !b.docs.every(docOk)
        )
            return null;
        for (const doc of b.docs) if (!validWidth(doc.readingWidth)) delete doc.readingWidth;
        if (b.temporary !== true) delete b.temporary;
        return b;
    } catch {
        return null;
    }
}

// ---- Short links ----
/**
 * Largest blob that the service stores, in base64url characters: slightly less than 1 MiB of bytes,
 * the KV value cap. Keep in sync with MAX_CHARS in the responder script of `index.html`.
 */
export const SHORT_MAX_CHARS = 1398100;

// SHA-256 over the concatenation of the parts. A string part counts as its UTF-8 bytes.
const sha256 = async (...parts: (string | Uint8Array)[]) =>
    new Uint8Array(await crypto.subtle.digest('SHA-256', await new Blob(parts as BlobPart[]).arrayBuffer()));

/**
 * Derives both halves of a short link from its secret.
 * - `id`: base64url of the first 12 bytes of SHA-256("id" || secret). The service stores the
 *   blob under it. It is the only derived value that leaves the browser.
 * - `key`: AES-256-GCM key from SHA-256("key" || secret), not extractable.
 * The hash is one-way, so the service cannot compute the secret or the key from `id`.
 * It does not check the secret length. `openSealed` does.
 */
export async function deriveShortLink(secret: Uint8Array) {
    const [idHash, keyBytes] = await Promise.all([sha256('id', secret), sha256('key', secret)]);
    const key = await crypto.subtle.importKey('raw', keyBytes as BufferSource, 'AES-GCM', false, [
        'encrypt',
        'decrypt',
    ]);
    return { id: toBase64Url(idHash.subarray(0, 12)), key };
}

/**
 * Encrypts a bundle for a short link with a fresh 12-byte secret and a fresh 12-byte IV.
 * Returns `id` and `blob` (base64url of IV || ciphertext) for the upload, and `secret`
 * (base64url) for the `#s=` fragment. The caller must never send `secret` to the service.
 */
export async function sealBundle(bundle: Bundle) {
    const secret = crypto.getRandomValues(new Uint8Array(12));
    const { id, key } = await deriveShortLink(secret);
    const iv = crypto.getRandomValues(new Uint8Array(12));
    const plain = await encodeStateBytes(JSON.stringify(bundle));
    const sealed = new Uint8Array(await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, plain as BufferSource));
    const envelope = new Uint8Array(iv.length + sealed.length);
    envelope.set(iv);
    envelope.set(sealed, iv.length);
    return { id, secret: toBase64Url(secret), blob: toBase64Url(envelope) };
}

/**
 * Decrypts a blob from the service with the secret from the fragment.
 * The blob is untrusted. Returns null when the secret is not 12 bytes, and when authentication
 * fails (a wrong key or a changed blob). Also returns null when the plaintext is too large or
 * is not a valid bundle.
 */
export async function openSealed(secretParam: string, blob: string): Promise<Bundle | null> {
    try {
        const secret = fromBase64Url(secretParam);
        if (secret.length !== 12) return null;
        const { key } = await deriveShortLink(secret);
        const envelope = fromBase64Url(blob);
        const plain = new Uint8Array(
            await crypto.subtle.decrypt({ name: 'AES-GCM', iv: envelope.subarray(0, 12) }, key, envelope.subarray(12)),
        );
        return parseBundle(await decodeStateBytes(plain));
    } catch {
        return null;
    }
}

/** Base64url SHA-256 of the UTF-8 text. It identifies equal content. It is not a secret. */
export const hashText = async (text: string) => toBase64Url(await sha256(text));
