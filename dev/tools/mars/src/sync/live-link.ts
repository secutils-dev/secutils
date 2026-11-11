// Only the derived invitation ID and bearer token go to the responder.
// The short fragment's encryption key is independent of that bearer token.
import { hash, openJson, sealJson } from './crypto.ts';
import { decode, encode, utf8, validId, SyncError } from './protocol.ts';

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
export interface LiveBootstrap {
    nonce: string;
    ciphertext: string;
}
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
export function linkWrappingKey(value: unknown) {
    if (!validLinkWrappingKey(value)) throw new SyncError('unsupported-link-key-format');
    return decode(value.wrappingKey);
}
export const isShortLiveLink = (value: string) => /^[A-Za-z0-9_-]{22}$/.test(value);
export const newLiveSecret = () => encode(crypto.getRandomValues(new Uint8Array(16)));
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
export const liveAccess = (secret: string) => invitationAccess(secret, 'mars-live-v1');
const context = (id: string) => ({ purpose: 'mars-live-bootstrap-v1', invitation: id });
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
