// Types of the Sync responder: the interfaces of the responder host (`Kv`, `Runtime`, `Context`)
// and the records that the responder stores. No code lives here, so every module can import it.
import type { CollectionRoot, WrappedKey, SignedInvitation, SignedCheckpoint, SignedGrant } from '../protocol.ts';

/** One row of a `Kv.list` page. */
export interface Entry {
    key: string;
    value?: string | null;
    createdAt?: number;
}
/**
 * The key-value store of the responder host. A value is a base64url string of at most 1 MiB of bytes.
 * `set` with `ifAbsent` keeps an existing live value, and `ttlSec` sets the expiry. A record
 * without `ttlSec` never expires. `list` returns keys in ascending order.
 */
export interface Kv {
    get(key: string): Promise<string | null>;
    getEntry(key: string): Promise<{ value: string | null; expiresAt: number | null }>;
    set(key: string, value: string, options?: { ifAbsent?: boolean; ttlSec?: number }): Promise<void>;
    delete(key: string): Promise<boolean>;
    list(options: {
        prefix: string;
        after?: string;
        limit: number;
        valuesIncluded?: boolean;
    }): Promise<{ entries: Entry[]; cursor: string | null }>;
}
/** Host services for one request. `sha256` takes base64url bytes and returns a lowercase hex digest. */
export interface Runtime {
    kv: Kv;
    crypto: { sha256(bytes: string): Promise<string> | string };
}
/** The HTTP request as the responder host presents it. `body` is the raw request body. */
export interface Context {
    path: string;
    method: string;
    query?: Record<string, string>;
    headers?: Record<string, string>;
    body: Uint8Array;
    clientAddress?: string;
    secrets?: Record<string, string>;
    params?: { secrets?: Record<string, string> };
}
/** The root record of a collection: the self-signed owner grant, and the root for a single-root collection. */
export interface Collection {
    v: 1;
    owner: SignedGrant;
    root?: CollectionRoot;
}
/** A member record: the signed grant, the membership slot that it holds, and its wrapped keys from pairing. */
export interface Member {
    signed: SignedGrant;
    slot: string;
    keys?: WrappedKey[];
}
/**
 * An invitation for a Sync link (`pair`) or a Live link (`live`). The service stores the hash of
 * the bearer token and, for a short link, a bootstrap that only the link secret decrypts.
 */
export interface InviteRecord {
    policy: SignedInvitation;
    tokenHash: string;
    inviter: string;
    bootstrap?: { nonce: string; ciphertext: string };
}
/**
 * A stored checkpoint header. `payload` names the separate ciphertext record, and `slots` lists
 * the capacity permits that the checkpoint holds until it is pruned or deleted.
 */
export interface Stored {
    envelope: SignedCheckpoint;
    author: SignedGrant;
    slots: string[];
    payload?: string;
}
/** One generation of a quota slot ledger. `busy` is false after a release. See `reserve`. */
export interface Permit {
    collection: string;
    operation: string;
    checkpoint: string;
    reservedAt: number;
    generation: number;
    busy: boolean;
}
/** A JSON request body. The handlers validate each field before use. */
export type SyncRequest = Record<string, any>;
