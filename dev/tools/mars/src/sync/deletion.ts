// Authenticated deletion receipts for personal Sync. Service flags and HTTP errors never authorize
// erasing local content. A receipt survives grant expiry; its original owner signature is the authority.
import { validGrant, validId, validBase64, type SignedGrant } from './protocol.ts';
import { verify, verifyGrant } from './crypto.ts';

/** A permanent deletion of one remote identity, signed by the deleting personal device. */
export interface SignedDeletion {
    deletion: {
        v: 1;
        kind: 'mars-sync-delete';
        collection: string;
        document: string;
        member: string;
        operation: string;
    };
    signature: string;
    author: SignedGrant;
}

/** Only the owner or a directly paired, whole-collection editor can delete shared identities. */
export const deletionAllowed = (author: SignedGrant, owner: SignedGrant) =>
    !author.issuer &&
    author.grant.collection === owner.grant.collection &&
    author.grant.document === null &&
    author.grant.participant === owner.grant.participant &&
    ['owner', 'edit'].includes(author.grant.permission);

/** Bounds an untrusted receipt and rejects extra fields before signature verification. */
export function validDeletion(value: unknown): value is SignedDeletion {
    if (!value || typeof value !== 'object') return false;
    const packet = value as SignedDeletion;
    const d = packet.deletion;
    return (
        Object.keys(packet).sort().join(',') === 'author,deletion,signature' &&
        !!d &&
        Object.keys(d).sort().join(',') === 'collection,document,kind,member,operation,v' &&
        d.v === 1 &&
        d.kind === 'mars-sync-delete' &&
        [d.collection, d.document, d.member, d.operation].every(validId) &&
        validBase64(packet.signature, 86) &&
        validGrant(packet.author) &&
        d.collection === packet.author.grant.collection &&
        d.member === packet.author.grant.member
    );
}

/** Verifies an immutable deletion against the trusted owner key. Failure never changes storage. */
export async function verifyDeletion(value: unknown, owner: SignedGrant): Promise<boolean> {
    return (
        validDeletion(value) &&
        deletionAllowed(value.author, owner) &&
        (await verifyGrant(value.author, owner.grant)) &&
        (await verify(value.deletion, value.signature, value.author.grant.publicKey))
    );
}
