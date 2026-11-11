// Read-only operator inventory. Ciphertexts are measured, never decrypted or returned.
// Small pages and sequential value reads bound both the KV budget and the sandbox heap.
import { decode, encode, fail, validId } from './protocol.ts';
import type { Runtime } from './responder/types.ts';

const PAGE_ENTRIES = 32;
const CURSOR_MAX_CHARS = 4096;
const METADATA_MAX_BYTES = 16 * 1024;
const HASH_DOMAIN = 'mars-storage-audit-v1:';

/** One measured KV record. Sizes are decoded value bytes; identifiers are one-way hashes. */
export interface StorageAuditRow {
    kind: string;
    bytes: number;
    expiresAt: number | null;
    createdAt?: number;
    collection?: string;
    invitation?: string;
    document?: string;
    liveUntil?: number;
    liveSource?: 'invitation' | 'grant';
    invitationMode?: 'live' | 'pair';
    problem?: 'invalid-metadata' | 'oversized-metadata';
}

/** A bounded, non-transactional inventory page. The private cursor must not enter logs or URLs. */
export interface StorageAuditPage {
    v: 1;
    at: number;
    rows: StorageAuditRow[];
    cursor: string | null;
    complete: boolean;
    scanned: number;
    missing: number;
}

const valueBytes = (value: string) => Math.floor((value.length * 3) / 4);
const metadata = (value: string) => JSON.parse(new TextDecoder().decode(decode(value)));

async function classify(key: string, value: string, runtime: Runtime): Promise<StorageAuditRow> {
    const parts = key.split('/');
    const row: StorageAuditRow = { kind: 'other', bytes: valueBytes(value), expiresAt: null };
    const hash = async (kind: string, id: string) => {
        if (!validId(id)) throw new Error('Invalid audit identifier');
        return runtime.crypto.sha256(encode(new TextEncoder().encode(`${HASH_DOMAIN}${kind}:${id}`)));
    };
    if (parts[0] === 'testing') {
        row.kind = parts[2] === 'scopes' ? 'test-audit' : 'test-data';
        return row;
    }
    const snapshotKinds: Record<string, string> = {
        s: 'snapshot-payload',
        d: 'snapshot-delete-token',
        rl: 'snapshot-rate-limit',
    };
    if (snapshotKinds[parts[0]]) {
        row.kind = snapshotKinds[parts[0]];
        return row;
    }
    if (parts[0] !== 'sync' || parts[1] !== 'v1') return row;

    let inspect: 'member' | 'invitation' | 'quota' | null = null;
    try {
        if (parts[2] === 'c' && validId(parts[3])) {
            row.collection = await hash('collection', parts[3]);
            const kinds: Record<string, string> = {
                owner: 'sync-collection',
                payloads: 'sync-payload',
                updates: 'sync-checkpoint-header',
                documents: 'sync-document-metadata',
                members: 'sync-member-grant',
                keys: 'sync-wrapped-key',
                epochs: 'sync-key-epoch',
                revoked: 'sync-revocation-fence',
                'revoked-invitations': 'sync-revocation-fence',
                deleted: 'sync-deletion-fence',
            };
            row.kind = kinds[parts[4]] ?? 'sync-collection-metadata';
            if (parts[4] === 'members') inspect = 'member';
        } else if (parts[2] === 'invitations' && validId(parts[3])) {
            row.kind = parts.length === 4 ? 'sync-invitation' : 'sync-invitation-support';
            row.invitation = await hash('invitation', parts[3]);
            if (parts.length === 4) inspect = 'invitation';
        } else if (parts[2] === 'budget') {
            row.kind = parts[3] === 'all' ? 'sync-global-quota-ledger' : 'sync-collection-quota-ledger';
            if (parts[3] === 'all') inspect = 'quota';
            else row.collection = await hash('collection', parts[3]);
        } else if (parts[2] === 'reservations' && validId(parts[3])) {
            row.kind = 'sync-reservation-reference';
            row.collection = await hash('collection', parts[3]);
        } else {
            row.kind = parts[2] === 'create' ? 'sync-creation-rate-limit' : 'sync-other';
        }
        if (!inspect) return row;
        if (row.bytes > METADATA_MAX_BYTES) {
            row.problem = 'oversized-metadata';
            return row;
        }
        const record = metadata(value);
        if (inspect === 'quota') {
            row.collection = await hash('collection', record.collection);
        } else if (inspect === 'invitation') {
            const policy = record.policy?.invitation;
            if (
                !policy ||
                policy.id !== parts[3] ||
                !['live', 'pair'].includes(policy.mode) ||
                !Number.isSafeInteger(policy.expires) ||
                policy.expires <= 0
            ) {
                throw new Error('Invalid invitation metadata');
            }
            row.collection = await hash('collection', policy.collection);
            row.invitationMode = policy.mode;
            if (policy.mode === 'live') {
                row.document = await hash('document', policy.document);
                row.liveUntil = policy.expires;
                row.liveSource = 'invitation';
            }
        } else {
            const grant = record.signed?.grant;
            if (
                !grant ||
                grant.collection !== parts[3] ||
                !Number.isSafeInteger(grant.expires) ||
                grant.expires < 0 ||
                !['owner', 'view', 'comment', 'edit'].includes(grant.permission)
            ) {
                throw new Error('Invalid member metadata');
            }
            if (grant.document !== null) {
                row.document = await hash('document', grant.document);
                if (grant.permission !== 'owner') {
                    row.liveUntil = grant.expires || Number.MAX_SAFE_INTEGER;
                    row.liveSource = 'grant';
                }
            }
        }
    } catch {
        row.problem = 'invalid-metadata';
    }
    return row;
}

/**
 * Read at most 32 records with at most 33 KV reads. No storage writes or deletes occur.
 * Invalid cursors fail with a fixed error code. Values are never returned; row identifiers are hashed.
 * The private pagination cursor encodes a storage key and must stay in the authenticated Node client.
 */
export async function auditStoragePage(request: Record<string, unknown>, runtime: Runtime): Promise<StorageAuditPage> {
    let after: string | undefined;
    if (request.after !== undefined && request.after !== null) {
        if (typeof request.after !== 'string' || !request.after.length || request.after.length > CURSOR_MAX_CHARS) {
            return fail('invalid-audit-cursor');
        }
        try {
            after = new TextDecoder().decode(decode(request.after));
        } catch {
            return fail('invalid-audit-cursor');
        }
        if (!after || after.length > CURSOR_MAX_CHARS) return fail('invalid-audit-cursor');
    }
    const at = Math.floor(Date.now() / 1000);
    const page = await runtime.kv.list({ prefix: '', after, limit: PAGE_ENTRIES, valuesIncluded: false });
    const rows: StorageAuditRow[] = [];
    let missing = 0;
    for (const item of page.entries) {
        let { value, expiresAt } = await runtime.kv.getEntry(item.key);
        if (value === null || (expiresAt !== null && expiresAt <= at)) {
            missing++;
            continue;
        }
        const row = await classify(item.key, value, runtime);
        value = '';
        row.expiresAt = expiresAt;
        if (item.createdAt !== undefined) row.createdAt = item.createdAt;
        rows.push(row);
    }
    const complete = page.entries.length < PAGE_ENTRIES;
    if (!complete && (!page.cursor || page.cursor === after)) return fail('audit-cursor-stalled', 503);
    return {
        v: 1,
        at,
        rows,
        cursor: complete ? null : encode(new TextEncoder().encode(page.cursor!)),
        complete,
        scanned: page.entries.length,
        missing,
    };
}
