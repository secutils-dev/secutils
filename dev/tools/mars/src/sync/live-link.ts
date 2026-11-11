// Only the derived invitation ID and bearer token go to the responder.
// The short fragment's encryption key is independent of that bearer token.
// A short link is one 16-byte secret in the URL fragment. Three SHA-256 values with separate purposes derive
// from it: the invitation ID, the bearer token, and the encryption key. One value does not reveal another.
// A bootstrap from the service is untrusted until it decrypts under that key and its fields match the derivation.
import { hash, openJson, sealJson } from './crypto.ts';
import { decode, encode, utf8, validId, SyncError } from './protocol.ts';

/**
 * Content of a Live link: what a recipient needs to join one document.
 * - `id`: invitation ID. `secret`: bearer token for the invitation endpoints.
 * - `ownerKey`: ECDSA public key of the owner. Each later signature check starts from it.
 * - `signing`: private JWK of the delegation key of the invitation. It signs the grant of the recipient.
 * - `key`, `epoch`: the document key (32 bytes in base64url) and its key epoch.
 * - `wrappingKey`: 32-byte key that decrypts later key updates for this link. It must differ from `secret`.
 */
export interface LiveLink {
    v: 1;
    id: string;
    secret: string;
    ownerKey: string;
    signing: JsonWebKey;
    key: string;
    epoch?: number;
    wrappingKey: string;
}
/** The encrypted link content that the service stores for a short link (`sealJson` output). */
export interface LiveBootstrap {
    nonce: string;
    ciphertext: string;
}
/**
 * True when a link has a 32-byte bearer secret and a different 32-byte wrapping key. The service sees the bearer
 * secret, so a link that used it as the wrapping key would expose key updates.
 */
export function validLinkWrappingKey(value: unknown): value is { secret: string; wrappingKey: string } {
    const link = value as { secret?: unknown; wrappingKey?: unknown } | null;
    try {
        return (
            typeof link?.secret === 'string' &&
            typeof link.wrappingKey === 'string' &&
            link.secret !== link.wrappingKey &&
            decode(link.secret).length === 32 &&
            decode(link.wrappingKey).length === 32
        );
    } catch {
        return false;
    }
}
/** Returns the wrapping key bytes of a link. Throws `SyncError` `unsupported-link-key-format` for an invalid link. */
export function linkWrappingKey(value: unknown) {
    if (!validLinkWrappingKey(value)) throw new SyncError('unsupported-link-key-format');
    return decode(value.wrappingKey);
}
/** True for the text form of a short link secret: 22 base64url characters. */
export const isShortLiveLink = (value: string) => /^[A-Za-z0-9_-]{22}$/.test(value);
/** Generates a short link secret: 16 random bytes as 22 base64url characters. */
export const newLiveSecret = () => encode(crypto.getRandomValues(new Uint8Array(16)));
/**
 * Derives the three values of a short link: SHA-256 of `<domain>:<purpose>:<secret>` for the purposes `id`, `access`,
 * and `encryption`.
 * - `id`: the first 16 bytes as a version 4 UUID. The service receives it.
 * - `token`: 32-byte bearer token in base64url. The service receives it.
 * - `key`: 32-byte AES-256-GCM key. It never enters a request.
 *
 * The domain separates Live links from Sync links, so one secret cannot serve both.
 * @throws `Error` for a secret that is not 16 bytes of canonical base64url.
 */
export async function invitationAccess(secret: string, domain: 'mars-live-v1' | 'mars-sync-v1') {
    if (!isShortLiveLink(secret) || decode(secret).length !== 16) throw new Error('Invalid live link');
    const derive = async (purpose: string) =>
        new Uint8Array(
            (await hash(utf8(`${domain}:${purpose}:${secret}`))).match(/../g)!.map((byte) => Number.parseInt(byte, 16)),
        );
    const identifier = await derive('id');
    identifier[6] = (identifier[6] & 15) | 64;
    identifier[8] = (identifier[8] & 63) | 128;
    const hex = [...identifier.subarray(0, 16)].map((byte) => byte.toString(16).padStart(2, '0')).join('');
    const id = `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
    return { id, token: encode(await derive('access')), key: await derive('encryption') };
}
/** Derives the values of a short Live link. */
export const liveAccess = (secret: string) => invitationAccess(secret, 'mars-live-v1');
/** Associated data of a Live bootstrap. It binds the ciphertext to its purpose and its invitation. */
const context = (id: string) => ({ purpose: 'mars-live-bootstrap-v1', invitation: id });
/**
 * Encrypts the content of a short Live link for storage on the service. It refuses a link whose ID, bearer secret, or
 * wrapping key differ from the values that `secret` derives.
 */
export async function sealLiveBootstrap(link: LiveLink, secret: string): Promise<LiveBootstrap> {
    const access = await liveAccess(secret);
    if (
        !validLinkWrappingKey(link) ||
        link.id !== access.id ||
        link.secret !== access.token ||
        link.wrappingKey !== encode(access.key)
    )
        throw new Error('Invalid live bootstrap');
    return sealJson(link, access.key, context(access.id));
}
/**
 * Decrypts and validates the bootstrap that the service returned for a short Live link. AES-GCM shows that a holder of
 * the link secret made it. The field checks tie its ID, bearer secret, and wrapping key to that secret and bound the
 * key material.
 * @throws A Web Crypto error for a failed decryption, or `Error` for a field that does not match.
 */
export async function openLiveBootstrap(bootstrap: LiveBootstrap, secret: string): Promise<LiveLink> {
    const access = await liveAccess(secret),
        link = await openJson<LiveLink>(bootstrap, access.key, context(access.id));
    if (
        !validLinkWrappingKey(link) ||
        link.v !== 1 ||
        !validId(link.id) ||
        link.id !== access.id ||
        link.secret !== access.token ||
        link.wrappingKey !== encode(access.key) ||
        typeof link.ownerKey !== 'string' ||
        !link.signing ||
        decode(link.key).length !== 32 ||
        !Number.isSafeInteger(link.epoch) ||
        link.epoch! < 1
    )
        throw new Error('Invalid live bootstrap');
    return link;
}
