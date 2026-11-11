// Native Web Crypto: independent ECDSA and ECDH P-256 keys, HKDF-SHA256, AES-256-GCM.
// This file never runs in the responder. No data key or private key leaves a client unencrypted.
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

export interface Identity {
    signing: JsonWebKey;
    box: JsonWebKey;
    publicKey: string;
    boxKey: string;
}
const P256 = { name: 'ECDSA', namedCurve: 'P-256' };
const ECDH = { name: 'ECDH', namedCurve: 'P-256' };
const SIGN = { name: 'ECDSA', hash: 'SHA-256' };
export const MAX_RECOVERY_FILE_BYTES = 16 * 1024 * 1024;
const MAX_RECOVERY_CIPHERTEXT = MAX_RECOVERY_FILE_BYTES - 1024;
export const randomSecret = () => encode(crypto.getRandomValues(new Uint8Array(32)));
export const keyBytes = () => crypto.getRandomValues(new Uint8Array(32));
export const hash = async (bytes: Uint8Array<ArrayBuffer>) =>
    [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))]
        .map((b) => b.toString(16).padStart(2, '0'))
        .join('');
export const tokenHash = (token: string) => hash(decode(token));
export const checkpointDigest = async (envelope: SignedCheckpoint) =>
    hash(jsonBytes(checkpointReceipt(envelope, await hash(decode(envelope.checkpoint.ciphertext)))));
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
export async function sign(value: unknown, who: Identity) {
    const privateKey = await crypto.subtle.importKey('jwk', who.signing, P256, false, ['sign']);
    return encode(new Uint8Array(await crypto.subtle.sign(SIGN, privateKey, jsonBytes(value))));
}
export async function verify(value: unknown, signature: string, publicKey: string) {
    try {
        const key = await crypto.subtle.importKey('raw', decode(publicKey), P256, false, ['verify']);
        return await crypto.subtle.verify(SIGN, key, decode(signature), jsonBytes(value));
    } catch {
        return false;
    }
}
export const signGrant = async (grant: Grant, owner: Identity): Promise<SignedGrant> => ({
    grant,
    signature: await sign(grant, owner),
});
export async function verifyGrant(signed: SignedGrant, owner: Grant) {
    if (!validGrant(signed)) return false;
    if (!signed.issuer) return verify(signed.grant, signed.signature, owner.publicKey);
    return (
        delegatedGrant(signed, owner) &&
        (await verify(signed.issuer.invitation, signed.issuer.signature, owner.publicKey)) &&
        (await verify(signed.grant, signed.signature, signed.issuer.invitation.publicKey))
    );
}
export async function sealJson(value: unknown, key: Uint8Array<ArrayBuffer>, context: unknown) {
    const nonce = crypto.getRandomValues(new Uint8Array(12));
    const ciphertext = await crypto.subtle.encrypt(
        { name: 'AES-GCM', iv: nonce, additionalData: jsonBytes(context) },
        await aes(key, ['encrypt']),
        jsonBytes(value),
    );
    return { nonce: encode(nonce), ciphertext: encode(new Uint8Array(ciphertext)) };
}
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
const aes = (bytes: Uint8Array<ArrayBuffer>, usage: KeyUsage[]) =>
    crypto.subtle.importKey('raw', bytes, 'AES-GCM', false, usage);
const context = (checkpoint: Checkpoint) => {
    const { nonce: _nonce, ciphertext: _ciphertext, ...metadata } = checkpoint;
    return jsonBytes(metadata);
};
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
export async function pairingPhrase(invitation: unknown, requestedGrant: Grant) {
    // 48 bits of the transcript digest, grouped for comparison. Neither party chooses the phrase.
    const digest = await hash(jsonBytes({ invitation, requestedGrant }));
    return digest.slice(0, 12).match(/.{4}/g)!.join(' ');
}
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
