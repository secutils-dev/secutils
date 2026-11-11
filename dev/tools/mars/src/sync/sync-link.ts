// Short Sync links for device pairing. They use the Live-link derivation with the separate `mars-sync-v1`
// domain and their own bootstrap purpose, so a Sync secret opens no Live invitation.
// The link alone grants nothing: the owner still approves the matching code.
import { openJson, sealJson } from './crypto.ts';
import { decode, encode } from './protocol.ts';
import {
    invitationAccess,
    isShortLiveLink,
    newLiveSecret,
    validLinkWrappingKey,
    type LiveBootstrap,
} from './live-link.ts';

/**
 * Content of a Sync link.
 * - `id`: invitation ID. `secret`: bearer token for the pairing endpoints.
 * - `ownerKey`: raw ECDSA public key of the owner (65 bytes in base64url). Signature checks of the pairing start from
 *   it.
 * - `wrappingKey`: 32-byte key for pairing data such as device labels. It must differ from `secret`.
 */
export interface PairLink {
    v: 1;
    id: string;
    secret: string;
    ownerKey: string;
    wrappingKey: string;
}
/** True for the text form of a short Sync link secret: 22 base64url characters. */
export const isShortSyncLink = isShortLiveLink;
/** Generates a short Sync link secret: 16 random bytes as 22 base64url characters. */
export const newSyncSecret = newLiveSecret;
/** Derives the invitation ID, the bearer token, and the encryption key of a short Sync link. */
export const syncAccess = (secret: string) => invitationAccess(secret, 'mars-sync-v1');
/** Associated data of a Sync bootstrap. It binds the ciphertext to its purpose and its invitation. */
const context = (id: string) => ({ purpose: 'mars-sync-bootstrap-v1', invitation: id });
/**
 * Encrypts the content of a short Sync link for storage on the service. It refuses a link whose ID, bearer secret, or
 * wrapping key differ from the values that `secret` derives.
 */
export async function sealSyncBootstrap(link: PairLink, secret: string) {
    const access = await syncAccess(secret);
    if (
        !validLinkWrappingKey(link) ||
        link.id !== access.id ||
        link.secret !== access.token ||
        link.wrappingKey !== encode(access.key)
    )
        throw new Error('Invalid Sync bootstrap');
    return sealJson(link, access.key, context(access.id));
}
/**
 * Decrypts and validates the bootstrap that the service returned for a short Sync link. The field checks tie its ID,
 * bearer secret, and wrapping key to the link secret. The owner key must be 65 bytes.
 * @throws A Web Crypto error for a failed decryption, or `Error` for a field that does not match.
 */
export async function openSyncBootstrap(bootstrap: LiveBootstrap, secret: string) {
    const access = await syncAccess(secret),
        link = await openJson<PairLink>(bootstrap, access.key, context(access.id));
    if (
        !validLinkWrappingKey(link) ||
        link.v !== 1 ||
        link.id !== access.id ||
        link.secret !== access.token ||
        link.wrappingKey !== encode(access.key) ||
        decode(link.ownerKey).length !== 65
    )
        throw new Error('Invalid Sync link');
    return link;
}
