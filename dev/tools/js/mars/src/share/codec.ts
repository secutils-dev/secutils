// Wire formats for sharing (pure; runs in browsers and Node >= 22 for tests).
//
// URL state: see dev/tools/AGENTS.md -> "URL state encoding":
//   base64url( u32le(uncompressed length) || deflate-raw(UTF-8 text) )
// Full links: `#name=<name>&md=<state>&o=<originId>` for one document (md.html-compatible;
// md.html ignores `o`), or `#b=<state of a bundle JSON>` for a folder.
// Short links: `#s=<secret>`; the responder stores base64url(IV || AES-GCM(state bytes of the
// bundle JSON)) under `id`. Both `id` and the AES key are SHA-256 hashes of the secret, so
// the key never leaves the fragment (same scheme as md.html, different plaintext).

const enc = new TextEncoder();
const dec = new TextDecoder();

export const toBase64Url = (bytes: Uint8Array) => {
    let s = '';
    for (const b of bytes) s += String.fromCharCode(b);
    return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
};

export const fromBase64Url = (str: string) => {
    const b64 = str.replace(/-/g, '+').replace(/_/g, '/');
    const bin = atob(b64 + '==='.slice(0, (4 - b64.length % 4) % 4));
    return Uint8Array.from(bin, (c) => c.charCodeAt(0));
};

async function pipe(bytes: Uint8Array, stream: CompressionStream | DecompressionStream) {
    return new Uint8Array(await new Response(new Blob([bytes as BlobPart]).stream().pipeThrough(stream)).arrayBuffer());
}

export async function encodeStateBytes(text: string) {
    const bytes = enc.encode(text);
    const deflated = await pipe(bytes, new CompressionStream('deflate-raw'));
    const out = new Uint8Array(4 + deflated.length);
    new DataView(out.buffer).setUint32(0, bytes.length, true);
    out.set(deflated, 4);
    return out;
}

export async function decodeStateBytes(bytes: Uint8Array) {
    if (bytes.length < 4) throw new Error('Truncated state');
    return dec.decode(await pipe(bytes.subarray(4), new DecompressionStream('deflate-raw')));
}

export const encodeState = async (text: string) => toBase64Url(await encodeStateBytes(text));
export const decodeState = async (str: string) => {
    try { return await decodeStateBytes(fromBase64Url(str)); } catch { return null; }
};

export interface SharedDoc {
    // Folder names from the shared root down to the document's parent.
    path: string[];
    name: string;
    originId: string;
    text: string;
}
export interface Bundle {
    v: 1;
    kind: 'doc' | 'folder';
    name: string;
    // The shared folder's originId, so a re-shared folder merges into the same copy.
    originId?: string;
    docs: SharedDoc[];
}

export function parseBundle(json: string): Bundle | null {
    try {
        const b = JSON.parse(json);
        const docOk = (d: SharedDoc) => d && typeof d.text === 'string' && typeof d.name === 'string'
            && typeof d.originId === 'string' && Array.isArray(d.path) && d.path.every((p) => typeof p === 'string');
        if (b?.v !== 1 || !['doc', 'folder'].includes(b.kind) || typeof b.name !== 'string' || !Array.isArray(b.docs) || !b.docs.every(docOk)) return null;
        return b;
    } catch { return null; }
}

// ---- Short links ----
// base64url length of 1 MiB, the KV value cap. Keep in sync with MAX_CHARS in mars.html's responder script.
export const SHORT_MAX_CHARS = 1398100;

const sha256 = async (...parts: (string | Uint8Array)[]) =>
    new Uint8Array(await crypto.subtle.digest('SHA-256', await new Blob(parts as BlobPart[]).arrayBuffer()));

export async function deriveShortLink(secret: Uint8Array) {
    const [idHash, keyBytes] = await Promise.all([sha256('id', secret), sha256('key', secret)]);
    const key = await crypto.subtle.importKey('raw', keyBytes as BufferSource, 'AES-GCM', false, ['encrypt', 'decrypt']);
    return { id: toBase64Url(idHash.subarray(0, 12)), key };
}

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

export async function openSealed(secretParam: string, blob: string): Promise<Bundle | null> {
    try {
        const secret = fromBase64Url(secretParam);
        if (secret.length !== 12) return null;
        const { key } = await deriveShortLink(secret);
        const envelope = fromBase64Url(blob);
        const plain = new Uint8Array(await crypto.subtle.decrypt({ name: 'AES-GCM', iv: envelope.subarray(0, 12) }, key, envelope.subarray(12)));
        return parseBundle(await decodeStateBytes(plain));
    } catch { return null; }
}

export const hashText = async (text: string) => toBase64Url(await sha256(text));
