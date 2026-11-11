// Native Web Crypto: independent ECDSA and ECDH P-256 keys, HKDF-SHA256, AES-256-GCM.
// This file never runs in the responder. No data key or private key leaves a client unencrypted.
// Each signature covers canonical JSON (`jsonBytes`), so the responder's P-256 verifier checks the same bytes.
// Each AES-GCM call here draws a random 12-byte nonce and binds its context as associated data.
// Decrypted JSON is authentic for its key and context. Its shape is still unchecked: the caller validates it.
import {
    decode,
    encode,
    fail,
    jsonBytes,
    utf8,
    delegatedGrant,
    validGrant,
    checkpointReceipt,
    type Checkpoint,
    type Grant,
    type SignedCheckpoint,
    type SignedGrant,
    type WrappedKey,
} from './protocol.ts';

/**
 * Key pairs of one member. `signing` (ECDSA) and `box` (ECDH) are private JWKs: treat the object as a secret.
 * `publicKey` and `boxKey` are the matching raw public keys in base64url, as a grant publishes them.
 */
export interface Identity {
    signing: JsonWebKey;
    box: JsonWebKey;
    publicKey: string;
    boxKey: string;
}
// Web Crypto algorithm parameters. Signing and key agreement use separate key pairs.
const P256 = { name: 'ECDSA', namedCurve: 'P-256' };
const ECDH = { name: 'ECDH', namedCurve: 'P-256' };
const SIGN = { name: 'ECDSA', hash: 'SHA-256' };
/** Largest version 1 (JSON) recovery file in bytes. A larger package uses the archive in recovery-archive.ts. */
export const MAX_RECOVERY_FILE_BYTES = 16 * 1024 * 1024;
/** Largest ciphertext of a version 1 file in base64url characters. The margin holds the other JSON fields. */
const MAX_RECOVERY_CIPHERTEXT = MAX_RECOVERY_FILE_BYTES - 1024;
/** Returns 32 random bytes as base64url text (43 characters), for example a bearer token. */
export const randomSecret = () => encode(crypto.getRandomValues(new Uint8Array(32)));
/** Returns 32 random bytes, for example an AES-256 document key. */
export const keyBytes = () => crypto.getRandomValues(new Uint8Array(32));
/** SHA-256 of `bytes` as 64 lowercase hex characters. */
export const hash = async (bytes: Uint8Array<ArrayBuffer>) =>
    [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))]
        .map((b) => b.toString(16).padStart(2, '0'))
        .join('');
/** Hash of a bearer token in the form that a grant stores. Throws `SyncError` for text that is not base64url. */
export const tokenHash = (token: string) => hash(decode(token));
/**
 * Acknowledgement digest of an envelope, computed locally. The responder returns the same digest for the same bytes
 * (`checkpointReceipt`). A matching digest shows that the service received this exact envelope.
 */
export const checkpointDigest = async (envelope: SignedCheckpoint) =>
    hash(jsonBytes(checkpointReceipt(envelope, await hash(decode(envelope.checkpoint.ciphertext)))));
/** Generates the identity of a member: independent ECDSA and ECDH P-256 key pairs. */
export async function identity(): Promise<Identity> {
    const [signing, box] = await Promise.all([
        crypto.subtle.generateKey(P256, true, ['sign', 'verify']),
        crypto.subtle.generateKey(ECDH, true, ['deriveBits']),
    ]);
    return {
        signing: await crypto.subtle.exportKey('jwk', signing.privateKey),
        box: await crypto.subtle.exportKey('jwk', box.privateKey),
        publicKey: encode(new Uint8Array(await crypto.subtle.exportKey('raw', signing.publicKey))),
        boxKey: encode(new Uint8Array(await crypto.subtle.exportKey('raw', box.publicKey))),
    };
}
/**
 * Signs the canonical JSON of `value` with ECDSA P-256 and SHA-256.
 * @returns The raw 64-byte signature in base64url.
 */
export async function sign(value: unknown, who: Identity) {
    const privateKey = await crypto.subtle.importKey('jwk', who.signing, P256, false, ['sign']);
    return encode(new Uint8Array(await crypto.subtle.sign(SIGN, privateKey, jsonBytes(value))));
}
/**
 * Verifies an ECDSA signature over the canonical JSON of `value`. Each argument can come from the service. It returns
 * `false` for a wrong signature, a malformed key or signature, or a value without canonical JSON. It never throws. The
 * result means something only when `publicKey` comes from a trusted source, such as the owner key in a link.
 */
export async function verify(value: unknown, signature: string, publicKey: string) {
    try {
        const key = await crypto.subtle.importKey('raw', decode(publicKey), P256, false, ['verify']);
        return await crypto.subtle.verify(SIGN, key, decode(signature), jsonBytes(value));
    } catch {
        return false;
    }
}
/** Signs a grant with the given identity and returns the wire form without an `issuer`. */
export const signGrant = async (grant: Grant, owner: Identity): Promise<SignedGrant> => ({
    grant,
    signature: await sign(grant, owner),
});
/**
 * Decides whether an untrusted grant is valid under the grant of the owner. `owner` must come from a trusted source. A
 * direct grant needs the signature of the owner. A delegated grant needs an owner-signed Live invitation, the signature
 * of the invitation key, and limits inside the invitation (`delegatedGrant`). It returns `false` on any failure. It
 * checks neither the clock nor revocation.
 */
export async function verifyGrant(signed: SignedGrant, owner: Grant) {
    if (!validGrant(signed)) return false;
    if (!signed.issuer) return verify(signed.grant, signed.signature, owner.publicKey);
    return (
        delegatedGrant(signed, owner) &&
        (await verify(signed.issuer.invitation, signed.issuer.signature, owner.publicKey)) &&
        (await verify(signed.grant, signed.signature, signed.issuer.invitation.publicKey))
    );
}
/**
 * Encrypts the canonical JSON of `value` with AES-256-GCM under a random 12-byte nonce. `context` becomes associated
 * data. It names the purpose and the record, so the result fails to open elsewhere.
 * @param key 32 raw key bytes.
 */
export async function sealJson(value: unknown, key: Uint8Array<ArrayBuffer>, context: unknown) {
    const nonce = crypto.getRandomValues(new Uint8Array(12));
    const ciphertext = await crypto.subtle.encrypt(
        { name: 'AES-GCM', iv: nonce, additionalData: jsonBytes(context) },
        await aes(key, ['encrypt']),
        jsonBytes(value),
    );
    return { nonce: encode(nonce), ciphertext: encode(new Uint8Array(ciphertext)) };
}
/**
 * Decrypts and authenticates a `sealJson` envelope from an untrusted source. A wrong key, a changed byte, or a
 * different `context` rejects with a Web Crypto error. Malformed base64url throws `SyncError`. The parsed value is
 * authentic for this key and context. Its shape is unchecked.
 */
export async function openJson<T>(
    value: { nonce: string; ciphertext: string },
    key: Uint8Array<ArrayBuffer>,
    context: unknown,
): Promise<T> {
    const bytes = await crypto.subtle.decrypt(
        { name: 'AES-GCM', iv: decode(value.nonce), additionalData: jsonBytes(context) },
        await aes(key, ['decrypt']),
        decode(value.ciphertext),
    );
    return JSON.parse(new TextDecoder().decode(bytes));
}
/** Imports raw bytes as a non-extractable AES-GCM key. */
const aes = (bytes: Uint8Array<ArrayBuffer>, usage: KeyUsage[]) =>
    crypto.subtle.importKey('raw', bytes, 'AES-GCM', false, usage);
/**
 * Associated data of a checkpoint: each field except the nonce and the ciphertext. It binds the ciphertext to its
 * collection, document, member, stream, epoch, sequence, and operation. A ciphertext that moves into a checkpoint with
 * other metadata fails to decrypt.
 */
const context = (checkpoint: Checkpoint) => {
    const { nonce: _nonce, ciphertext: _ciphertext, ...metadata } = checkpoint;
    return jsonBytes(metadata);
};
/**
 * Encrypts `body` under the document key and signs the complete checkpoint, ciphertext included. Each call draws a
 * fresh nonce. The caller must save the result and send those exact bytes again on a retry.
 */
export async function sealCheckpoint(
    metadata: Omit<Checkpoint, 'nonce' | 'ciphertext'>,
    body: Uint8Array<ArrayBuffer>,
    dataKey: Uint8Array<ArrayBuffer>,
    who: Identity,
): Promise<SignedCheckpoint> {
    const checkpoint: Checkpoint = {
        ...metadata,
        nonce: encode(crypto.getRandomValues(new Uint8Array(12))),
        ciphertext: '',
    };
    checkpoint.ciphertext = encode(
        new Uint8Array(
            await crypto.subtle.encrypt(
                { name: 'AES-GCM', iv: decode(checkpoint.nonce), additionalData: context(checkpoint) },
                await aes(dataKey, ['encrypt']),
                body,
            ),
        ),
    );
    return { checkpoint, signature: await sign(checkpoint, who) };
}
/**
 * Verifies and decrypts a downloaded checkpoint. The service and the author control the envelope. The envelope must
 * name the collection and member of the grant, stay inside a single-document grant, and carry a valid signature of the
 * grant key. AES-GCM then authenticates the metadata and the content. The caller verifies `grant` first (`verifyGrant`)
 * and checks the stream permission (`permitted`).
 * @throws `SyncError` `invalid-checkpoint-signature` or `invalid-checkpoint-encryption`.
 */
export async function openCheckpoint(
    envelope: SignedCheckpoint,
    dataKey: Uint8Array<ArrayBuffer>,
    grant: Grant,
): Promise<Uint8Array<ArrayBuffer>> {
    const c = envelope.checkpoint;
    if (
        c.collection !== grant.collection ||
        c.member !== grant.member ||
        (grant.document !== null && grant.document !== c.document) ||
        !(await verify(c, envelope.signature, grant.publicKey))
    )
        return fail('invalid-checkpoint-signature');
    try {
        return new Uint8Array(
            await crypto.subtle.decrypt(
                { name: 'AES-GCM', iv: decode(c.nonce), additionalData: context(c) },
                await aes(dataKey, ['decrypt']),
                decode(c.ciphertext),
            ),
        );
    } catch {
        return fail('invalid-checkpoint-encryption');
    }
}
/**
 * Derives the AES-256-GCM key that wraps one document key: ECDH P-256 between the two box keys, then HKDF-SHA256 with a
 * fixed salt. `metadata` is the HKDF info, so each collection, document, epoch, member, and sender has its own key.
 * Sender and recipient derive the same key.
 */
async function wrappingKey(who: Identity, publicKey: string, metadata: unknown) {
    const key = await crypto.subtle.importKey('jwk', who.box, ECDH, false, ['deriveBits']);
    const peer = await crypto.subtle.importKey('raw', decode(publicKey), ECDH, false, []);
    const shared = await crypto.subtle.deriveBits({ name: 'ECDH', public: peer }, key, 256);
    const material = await crypto.subtle.importKey('raw', shared, 'HKDF', false, ['deriveKey']);
    return crypto.subtle.deriveKey(
        { name: 'HKDF', hash: 'SHA-256', salt: utf8('MARS sync key wrapping v1'), info: jsonBytes(metadata) },
        material,
        { name: 'AES-GCM', length: 256 },
        false,
        ['encrypt', 'decrypt'],
    );
}
/**
 * Encrypts a document key for the box key of one member. The associated data repeats the HKDF info: version,
 * collection, document, epoch, member, and sender key.
 * @param recipient Raw ECDH public key of the member in base64url. Take it from a verified grant.
 */
export async function wrapKey(
    metadata: Pick<WrappedKey, 'collection' | 'document' | 'epoch' | 'member'>,
    dataKey: Uint8Array<ArrayBuffer>,
    who: Identity,
    recipient: string,
): Promise<WrappedKey> {
    const aad = { v: 1 as const, ...metadata, sender: who.boxKey },
        nonce = crypto.getRandomValues(new Uint8Array(12));
    const ciphertext = await crypto.subtle.encrypt(
        { name: 'AES-GCM', iv: nonce, additionalData: jsonBytes(aad) },
        await wrappingKey(who, recipient, aad),
        dataKey,
    );
    return { ...aad, nonce: encode(nonce), ciphertext: encode(new Uint8Array(ciphertext)) };
}
/**
 * Decrypts a wrapped document key with the box key of this member. `wrapped.sender` is untrusted. Success proves only
 * that the holder of that key made the envelope. Before use, the caller verifies the signature of the owner over
 * `wrapped`, or compares `sender` with the box key of the owner.
 * @throws `SyncError` `invalid-key-envelope` on any failure.
 */
export async function unwrapKey(wrapped: WrappedKey, who: Identity): Promise<Uint8Array<ArrayBuffer>> {
    const { nonce, ciphertext, ...aad } = wrapped;
    try {
        return new Uint8Array(
            await crypto.subtle.decrypt(
                { name: 'AES-GCM', iv: decode(nonce), additionalData: jsonBytes(aad) },
                await wrappingKey(who, wrapped.sender, aad),
                decode(ciphertext),
            ),
        );
    } catch {
        return fail('invalid-key-envelope');
    }
}
/**
 * Short authentication string for device pairing. Both devices derive it from the invitation and the requested grant,
 * and the user compares the two. A service that changes either value changes the phrase.
 * @returns 12 hex digits in three groups of four.
 */
export async function pairingPhrase(invitation: unknown, requestedGrant: Grant) {
    // 48 bits of the transcript digest, grouped for comparison. Neither party chooses the phrase.
    const digest = await hash(jsonBytes({ invitation, requestedGrant }));
    return digest.slice(0, 12).match(/.{4}/g)!.join(' ');
}
/**
 * Derives the AES-256-GCM recovery key from a password: PBKDF2-SHA256 with 600,000 iterations. The salt is random on
 * encryption. On decryption it comes from the untrusted file.
 */
export async function recoveryKey(password: string, salt: Uint8Array<ArrayBuffer>) {
    const material = await crypto.subtle.importKey('raw', utf8(password), 'PBKDF2', false, ['deriveKey']);
    return crypto.subtle.deriveKey(
        { name: 'PBKDF2', hash: 'SHA-256', iterations: 600000, salt },
        material,
        { name: 'AES-GCM', length: 256 },
        false,
        ['encrypt', 'decrypt'],
    );
}
/**
 * Encrypts a version 1 recovery package into JSON-safe fields. The password needs at least 12 characters. A fixed
 * purpose string is the associated data, so the ciphertext cannot pass as another format.
 * @throws `SyncError` `recovery-password-too-short` or `recovery-package-too-large`.
 */
export async function encryptRecovery(value: unknown, password: string) {
    if (password.length < 12) return fail('recovery-password-too-short');
    const plaintext = utf8(JSON.stringify(value));
    if (plaintext.length + 16 > (MAX_RECOVERY_CIPHERTEXT * 3) / 4) return fail('recovery-package-too-large');
    const salt = crypto.getRandomValues(new Uint8Array(16)),
        nonce = crypto.getRandomValues(new Uint8Array(12));
    const encrypted = await crypto.subtle.encrypt(
        { name: 'AES-GCM', iv: nonce, additionalData: utf8('MARS encrypted recovery v1') },
        await recoveryKey(password, salt),
        plaintext,
    );
    return {
        v: 1,
        kind: 'mars-encrypted-recovery',
        salt: encode(salt),
        nonce: encode(nonce),
        ciphertext: encode(new Uint8Array(encrypted)),
    };
}
/**
 * Decrypts a version 1 recovery package from an untrusted file. It checks the version, the kind, and the size before
 * key derivation. A wrong password or a changed file rejects with a Web Crypto error. The parsed value is authentic for
 * the password. Its shape is unchecked: the caller validates it before any write.
 * @throws `SyncError` `invalid-recovery-package` for a wrong version, kind, or size.
 */
export async function decryptRecovery<T>(
    value: { v: number; kind: string; salt: string; nonce: string; ciphertext: string },
    password: string,
): Promise<T> {
    if (value.v !== 1 || value.kind !== 'mars-encrypted-recovery' || value.ciphertext.length > MAX_RECOVERY_CIPHERTEXT)
        return fail('invalid-recovery-package');
    const bytes = await crypto.subtle.decrypt(
        { name: 'AES-GCM', iv: decode(value.nonce), additionalData: utf8('MARS encrypted recovery v1') },
        await recoveryKey(password, decode(value.salt)),
        decode(value.ciphertext),
    );
    return JSON.parse(new TextDecoder().decode(bytes));
}
