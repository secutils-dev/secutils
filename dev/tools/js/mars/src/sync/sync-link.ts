import { openJson, sealJson } from './crypto.ts';
import { decode, encode } from './protocol.ts';
import { invitationAccess, isShortLiveLink, newLiveSecret, type LiveBootstrap } from './live-link.ts';

export interface PairLink { v: 1; id: string; secret: string; ownerKey: string; wrappingKey?: string }
export const isShortSyncLink = isShortLiveLink;
export const newSyncSecret = newLiveSecret;
export const syncAccess = (secret: string) => invitationAccess(secret, 'mars-sync-v1');
const context = (id: string) => ({ purpose: 'mars-sync-bootstrap-v1', invitation: id });
export async function sealSyncBootstrap(link: PairLink, secret: string) {
    const access = await syncAccess(secret);
    if (link.id !== access.id || link.secret !== access.token || link.wrappingKey !== encode(access.key)) throw new Error('Invalid Sync bootstrap');
    return sealJson(link, access.key, context(access.id));
}
export async function openSyncBootstrap(bootstrap: LiveBootstrap, secret: string) {
    const access = await syncAccess(secret), link = await openJson<PairLink>(bootstrap, access.key, context(access.id));
    if (link.v !== 1 || link.id !== access.id || link.secret !== access.token || link.wrappingKey !== encode(access.key) || decode(link.ownerKey).length !== 65) throw new Error('Invalid Sync link');
    return link;
}
