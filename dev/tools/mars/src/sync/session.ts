// A connected document: body CRDT, author-separated reviews, and a durable outbox.
// Plain local state commits first. Ciphertexts remain immutable until acknowledgement.
// Each operation starts from the stored record and commits with a revision check, so tabs can share one record.
// Received checkpoints are untrusted. `receive` verifies the author, the signature, the key epoch, and the content.
// This module owns no network and no UI. transport.ts moves envelopes. The manager supplies the repository.
import * as Y from 'yjs';
import { BodyReplica, sourceChange } from './replica.ts';
import { ReviewReplica, signReview, type SignedReview, type ReviewOperation } from './reviews.ts';
import { commentKey, joinComments, splitComments, type Thread } from '../share/merge.ts';
import {
    decode,
    encode,
    fail,
    jsonBytes,
    canonical,
    permitted,
    RejectedPacket,
    SyncError,
    type SyncBinding,
    type SignedCheckpoint,
    type SignedGrant,
    type Stream,
} from './protocol.ts';
import { checkpointDigest, hash, openCheckpoint, sealCheckpoint, verifyGrant, type Identity } from './crypto.ts';

/**
 * One queued upload. `bytes` is the plaintext payload in base64url. `envelope` stays `null` until `prepare` seals the
 * packet. A sealed packet keeps its exact ciphertext until the acknowledgement. Only a key rotation (`rekey`) discards
 * that ciphertext.
 */
export interface PendingPacket {
    operation: string;
    sequence: number;
    stream: Stream;
    bytes: string;
    envelope: SignedCheckpoint | null;
}
/**
 * Durable state of one connected document. It holds secrets (`identity`, `token`, `key`, `keys`).
 * - `revision`: optimistic-concurrency counter. Each commit increments it.
 * - `token`: bearer token of this member. `owner`, `member`: signed grants of the collection owner and of this client.
 * - `key`: document key of `binding.epoch` in base64url. `keys`: document keys by epoch number, for older checkpoints.
 * - `body`: Yjs state in base64url. `projection`: the Markdown text that the body and the reviews produce.
 * - `seed`: review threads of the original text. `suffix`: the exact text of their comment block.
 * - `creator`: member that seeded the document. Only its review checkpoints can carry `seed` and `suffix`.
 * - `reviews`: accepted review operations by author member, each with the grant that verified them.
 * - `outbox`: uploads in order. `nextSequence`: sequence of the next packet.
 * - `progress`: highest applied sequence for each `<member>/<stream>`.
 * - `ended`: the connection is closed and the local copy remains.
 * - `acknowledgements`: digests of the latest 64 acknowledged operations, by operation ID.
 */
export interface SessionRecord {
    v: 1;
    kind: 'connected';
    revision: number;
    binding: SyncBinding;
    identity: Identity;
    token: string;
    key: string;
    owner: SignedGrant;
    member: SignedGrant;
    body: string;
    projection: string;
    seed: Thread[];
    suffix: string;
    creator?: string;
    reviews: Record<string, { grant: SignedGrant; operations: SignedReview[] }>;
    outbox: PendingPacket[];
    nextSequence: number;
    progress: Record<string, number>;
    ended?: boolean;
    keys?: Record<string, string>;
    acknowledgements?: Record<string, string>;
}
/**
 * Storage of one session record. `commit` saves atomically. It rejects with an error named `ReplicaConflict` when the
 * stored revision differs from `expected`. `remote` marks a commit of received state, as opposed to a local edit.
 */
export interface SessionRepository {
    get(): Promise<SessionRecord | undefined>;
    commit(expected: number | null, value: SessionRecord, remote?: boolean): Promise<void>;
}
/**
 * One entry of a folder manifest. `parent` is `null` for a child of the scope root. `key` and `epoch` carry the
 * document key of a child document. Editors write entries into the shared replica, so the content is untrusted.
 * `materializable` validates it.
 */
export interface ManifestNode {
    id: string;
    kind: 'doc' | 'folder';
    parent: string | null;
    name: string;
    order: number;
    key?: string;
    origin?: string;
    epoch?: number;
}
/**
 * A client can download its own older review checkpoint, for example after a lost acknowledgement
 * or a race with another tab. That echo must not replace newer local operations that still wait
 * in the outbox, so the local operations that the echo lacks stay in the list.
 */
function mergeOwnOperations(record: SessionRecord, author: SignedGrant, received: SignedReview[]) {
    if (author.grant.member !== record.member.grant.member) return received;
    const echoed = new Set(received.map((signed) => signed.operation.id));
    const local = record.reviews[author.grant.member]?.operations ?? [];
    return [...received, ...local.filter((signed) => !echoed.has(signed.operation.id))];
}
/**
 * Creates a session over a private in-memory repository. Incoming replicas stay private until their entire folder can
 * be committed.
 */
export function stagedSession(initial: SessionRecord) {
    let record = structuredClone(initial);
    return new Session(record, {
        get: async () => structuredClone(record),
        commit: async (expected, value) => {
            if (record.revision !== expected) throw new Error('Staged replica changed');
            record = structuredClone(value);
        },
    });
}
/** JSON round trip. It drops `undefined` fields, so a value compares and signs in its stored form. */
const normalized = <T>(value: T): T => JSON.parse(JSON.stringify(value));
/** Deep equality by canonical JSON, independent of key order. */
const same = (a: unknown, b: unknown) => canonical(normalized(a)) === canonical(normalized(b));
/**
 * Bounds an untrusted review seed: at most 1000 threads, each with a string ID, a quote of at most 8192 characters, and
 * at most 1000 comments with string bodies.
 */
const validSeed = (value: unknown): value is Thread[] =>
    Array.isArray(value) &&
    value.length <= 1000 &&
    value.every(
        (t) =>
            t &&
            typeof t.id === 'string' &&
            typeof t.quote === 'string' &&
            t.quote.length <= 8192 &&
            Array.isArray(t.comments) &&
            t.comments.length <= 1000 &&
            t.comments.every((c: unknown) => !!c && typeof (c as { body?: unknown }).body === 'string'),
    );

/**
 * One connected document in this tab.
 * - `replica` mirrors the committed record. `presented` mirrors the text that the editor displays.
 * - `record` is the latest committed snapshot that this session saw. Storage can be ahead after a commit in another
 *   tab.
 *
 * Public operations run one at a time. Call `destroy` to release both Yjs documents.
 */
export class Session {
    readonly replica: BodyReplica;
    readonly presented: BodyReplica;
    readonly repository: SessionRepository;
    record: SessionRecord;
    // Tail of the operation queue.
    private tasks: Promise<unknown> = Promise.resolve();
    // Text and reviews that the editor last showed. Local changes are computed against them.
    private presentedText: string;
    private presentedReviews: SessionRecord['reviews'];
    // Review operations from the editor that no committed record contains yet.
    private localReviewIntents: ReviewOperation[] = [];
    // True after a failed local commit. The next `change` then queues the complete body again.
    private failedLocal = false;
    constructor(record: SessionRecord, repository: SessionRepository) {
        this.record = structuredClone(record);
        this.repository = repository;
        this.replica = new BodyReplica(decode(record.body));
        this.presented = new BodyReplica(decode(record.body));
        this.presentedText = record.projection;
        this.presentedReviews = structuredClone(record.reviews);
    }
    /**
     * Builds the first record of a document for its creator: a seeded CRDT with the body of `text`, and the comment
     * block of `text` as `seed` and `suffix`. It stores nothing and queues no upload. The record names this member as
     * `creator`. A joiner replaces `body` and `creator` before it loads checkpoints.
     * @param key Document key of `binding.epoch` in base64url.
     * @param member Grant of this client. The default is the grant of the owner.
     */
    static seed(
        binding: SyncBinding,
        text: string,
        who: Identity,
        token: string,
        key: string,
        owner: SignedGrant,
        name: string,
        member = owner,
    ): SessionRecord {
        const parsed = splitComments(text),
            replica = new BodyReplica();
        replica.seed(parsed.body);
        replica.doc.getMap('meta').set('name', name);
        const value: SessionRecord = {
            v: 1,
            kind: 'connected',
            revision: 1,
            binding,
            identity: who,
            token,
            key,
            owner,
            member,
            creator: member.grant.member,
            body: encode(replica.checkpoint()),
            projection: text,
            seed: parsed.threads,
            suffix: text.slice(parsed.body.length),
            reviews: {},
            outbox: [],
            nextSequence: 1,
            progress: {},
        };
        replica.destroy();
        return value;
    }
    /**
     * Runs `run` after each earlier operation of this session, including failed ones. A `ReplicaConflict` means that
     * another tab committed first. `run` then starts again from the stored record, for at most five attempts.
     */
    private serial<T>(run: () => Promise<T>) {
        const task = this.tasks.then(async () => {
            for (let attempt = 0; ; attempt++) {
                try {
                    return await run();
                } catch (error) {
                    if ((error as Error)?.name !== 'ReplicaConflict' || attempt >= 4) throw error;
                }
            }
        });
        this.tasks = task.catch(() => {});
        return task;
    }
    /** Reads the stored record. Fails with `connection-ended` (409) for a missing, ended, or foreign record. */
    private async latest() {
        const stored = await this.repository.get();
        if (!stored || stored.ended || stored.binding.member !== this.record.binding.member)
            return fail('connection-ended', 409);
        return stored;
    }
    /** Creates an empty review projection for `record` and lists the stored review packets to apply. */
    private reviews(record = this.record) {
        const result = new ReviewReplica(record.binding.document, record.seed, record.owner.grant.participant);
        return { result, packets: Object.values(record.reviews) };
    }
    /**
     * Builds the Markdown projection: the body text plus the threads that the stored review operations produce. Each
     * operation passes `ReviewReplica.apply` again under the grant of its author. When the threads equal the seed, the
     * original comment-block text (`suffix`) is used again, so an unchanged document keeps its exact text. A CRLF body
     * keeps CRLF line endings.
     */
    private async projected(record: SessionRecord, replica = this.replica) {
        const { result, packets } = this.reviews(record);
        // Each outer checkpoint carries only its actual author's operations. Forwarding
        // another author's operation cannot bypass revocation or claim that author's identity.
        for (const packet of packets) for (const op of packet.operations) await result.apply(op, packet.grant.grant);
        const body = replica.text(),
            threads = result.threads();
        if (same(threads, record.seed)) {
            if (record.suffix) return body + record.suffix;
            if (!splitComments(body).threads.length) return body;
        }
        // Without threads, an explicit empty block ends the body. Body text that looks like a comment block stays body
        // text.
        const joined = threads.length
            ? joinComments(body, threads)
            : `${body}${body.endsWith('\n') ? '' : '\n'}\n<!-- su-comments\n[]\n-->\n`;
        return body.includes('\r\n') ? joined.replace(/\r?\n/g, '\r\n') : joined;
    }
    /**
     * Adds a full-state payload to the outbox of `record`. It replaces the payload of an unsealed packet of the same
     * stream, because the later state contains the earlier one. A sealed packet never changes.
     */
    private queue(record: SessionRecord, stream: Stream, bytes: Uint8Array) {
        const encoded = encode(bytes),
            pending = structuredClone(record.outbox),
            tail = pending.findLast((p) => p.stream === stream && !p.envelope);
        if (tail) tail.bytes = encoded;
        else
            pending.push({
                operation: crypto.randomUUID(),
                sequence: record.nextSequence++,
                stream,
                bytes: encoded,
                envelope: null,
            });
        record.outbox = pending;
    }
    /**
     * Review payload of this member: all of its signed operations. The document creator also sends the seed threads and
     * their original text.
     */
    private reviewBytes(record: SessionRecord) {
        return jsonBytes({
            operations: record.reviews[record.member.grant.member]?.operations ?? [],
            ...(record.member.grant.member === (record.creator ?? record.owner.grant.member)
                ? { seed: record.seed, suffix: record.suffix }
                : {}),
        });
    }
    /**
     * Queues the first checkpoints of a document: the tree state of a folder, or the body and review state of a
     * document. It commits the record. The transport uploads later.
     */
    async initial() {
        return this.serial(async () => {
            const record = await this.latest();
            this.queue(record, this.folder() ? 'tree' : 'body', this.replica.checkpoint());
            if (!this.folder()) this.queue(record, 'review', this.reviewBytes(record));
            await this.save(record);
        });
    }
    /** True when the shared metadata marks a folder. Any editor can write that flag: do not authorize with it. */
    folder() {
        return this.replica.doc.getMap('meta').get('kind') === 'folder';
    }
    /** Shared metadata of the replica. Any editor can write it: validate each field before use. */
    metadata() {
        return this.replica.doc.getMap('meta').toJSON() as {
            name?: string;
            readingWidth?: number | 'full';
            kind?: string;
            origin?: string;
        };
    }
    /** Entries of the folder manifest in the shared replica. They are untrusted until `materializable` filters them. */
    manifest(): ManifestNode[] {
        const entries = this.replica.doc.getMap<Y.Map<unknown>>('nodes');
        return [...entries.entries()].flatMap(([id, value]) =>
            value instanceof Y.Map ? [{ id, ...value.toJSON() } as ManifestNode] : [],
        );
    }
    /**
     * Merges entries into the folder manifest, field by field, and queues a tree checkpoint. A field that is
     * `undefined` keeps its stored value. It checks no permission. The responder rejects a tree upload without a
     * whole-collection edit grant.
     */
    async editManifest(values: ManifestNode[]) {
        return this.serial(async () => {
            const record = await this.latest();
            this.replica.apply(decode(record.body));
            this.replica.doc.transact(() => {
                const entries = this.replica.doc.getMap<Y.Map<unknown>>('nodes');
                for (const value of values) {
                    let item = entries.get(value.id);
                    if (!(item instanceof Y.Map)) {
                        item = new Y.Map();
                        entries.set(value.id, item);
                    }
                    for (const [key, field] of Object.entries(value))
                        if (key !== 'id' && field !== undefined && item.get(key) !== field) item.set(key, field);
                }
            }, this.replica.local);
            this.queue(record, 'tree', this.replica.checkpoint());
            await this.save(record);
        });
    }
    /**
     * Sets shared metadata fields and queues a tree checkpoint for a folder or a body checkpoint for a document.
     * @throws `SyncError` `metadata-read-only` (403) without write permission for that stream.
     */
    async metadataChanged(values: Record<string, string | number | null>) {
        return this.serial(async () => {
            const record = await this.latest();
            this.replica.apply(decode(record.body));
            if (!permitted(record.member.grant, record.binding.document, this.folder() ? 'tree' : 'body'))
                return fail('metadata-read-only', 403);
            this.replica.doc.transact(() => {
                const meta = this.replica.doc.getMap('meta');
                for (const [key, value] of Object.entries(values)) meta.set(key, value);
            }, this.replica.local);
            this.queue(record, this.folder() ? 'tree' : 'body', this.replica.checkpoint());
            await this.save(record);
        });
    }
    /** Commits a local change: takes body and projection from `replica` and writes the record at the next revision. */
    private async save(record: SessionRecord) {
        record.body = encode(this.replica.checkpoint());
        record.projection = await this.projected(record);
        record.revision++;
        await this.repository.commit(record.revision - 1, record);
        this.record = structuredClone(record);
    }
    /**
     * Records a local edit of the displayed document. The synchronous part applies the body change to `presented` and
     * turns the thread difference into review operations. The next call therefore builds on this one before storage
     * answers. The queued part merges that state into the latest stored record, signs the review operations, and
     * commits body, reviews, outbox, and projection together.
     * @param before Unused. The baseline is the text that this session last presented.
     * @param after The complete Markdown text of the editor, comment block included.
     * @returns The committed projection. It can also contain concurrent remote changes.
     * @throws `SyncError` `body-read-only` or `reviews-read-only` (403) when the grant forbids the change.
     */
    async change(before: string, after: string) {
        // Stage against the actual editor's CRDT synchronously. A remote transaction can
        // commit while IndexedDB is busy without making the next keystroke replace it.
        const old = splitComments(this.failedLocal ? this.record.projection : this.presentedText),
            next = splitComments(after);
        const change = sourceChange(splitComments(this.presentedText).body, next.body);
        if (change) {
            if (!permitted(this.record.member.grant, this.record.binding.document, 'body'))
                return fail('body-read-only', 403);
            this.presented.edit(change);
        }
        const staged = this.presented.checkpoint();
        this.presentedText = after;
        // Parents come from what this editor actually saw, not a newer unseen peer
        // version that happened to reach IndexedDB while the user typed.
        const causal = new ReviewReplica(
            this.record.binding.document,
            this.record.seed,
            this.record.owner.grant.participant,
        );
        for (const packet of Object.values(this.presentedReviews))
            for (const op of packet.operations) causal.operations.set(op.operation.id, op);
        for (const operation of this.localReviewIntents)
            causal.operations.set(operation.id, { operation, signature: '' });
        const appendIntent = (
            kind: ReviewOperation['kind'],
            thread: string,
            comment: string | null,
            value: ReviewOperation['value'],
        ) => {
            const previous = causal.heads(thread, comment, kind).map((p) => p.operation.id);
            const lastByActor = new Map<string, string>();
            // A reply follows the latest operation from every author it actually saw
            // in this thread, not only the replying author's previous operation.
            for (const { operation } of causal.operations.values())
                if (operation.thread === thread) lastByActor.set(operation.actor, operation.id);
            previous.push(...lastByActor.values());
            const operation: ReviewOperation = {
                v: 1,
                id: crypto.randomUUID(),
                document: this.record.binding.document,
                actor: this.record.member.grant.member,
                participant: this.record.member.grant.participant,
                previous: [...new Set(previous)],
                kind,
                thread,
                comment,
                value: normalized(value),
            };
            this.localReviewIntents.push(operation);
            causal.operations.set(operation.id, { operation, signature: '' });
        };
        // Translate the thread difference into typed operations: thread fields, resolution, comment versions,
        // deletions.
        if (!same(old.threads, next.threads)) {
            for (const thread of next.threads) {
                const previous = old.threads.find((t) => t.id === thread.id);
                const fields = (t: Thread) => ({ ...t, comments: [], resolved: undefined });
                if (!previous || !same(fields(previous), fields(thread)))
                    appendIntent('thread', thread.id, null, fields(thread));
                if (!!previous?.resolved !== !!thread.resolved)
                    appendIntent('resolve', thread.id, null, !!thread.resolved);
                for (const comment of thread.comments) {
                    const key = commentKey(comment),
                        oldComment = previous?.comments.find((c) => commentKey(c) === key);
                    if (!oldComment || !same(oldComment, comment))
                        appendIntent('comment', thread.id, key, { ...comment, id: key });
                }
                for (const comment of previous?.comments ?? [])
                    if (!thread.comments.some((c) => commentKey(c) === commentKey(comment)))
                        appendIntent('delete-comment', thread.id, commentKey(comment), null);
            }
            for (const thread of old.threads)
                if (!next.threads.some((t) => t.id === thread.id)) appendIntent('delete-thread', thread.id, null, null);
        }
        // Copy the list. A later `change` call can add intents before this task runs.
        const intents = [...this.localReviewIntents];
        return this.serial(async () => {
            const record = await this.latest();
            this.replica.apply(decode(record.body));
            if (change) {
                if (!permitted(record.member.grant, record.binding.document, 'body'))
                    return fail('body-read-only', 403);
            }
            this.replica.applyLocal(staged);
            if (change || this.failedLocal) this.queue(record, 'body', this.replica.checkpoint());
            // Sign only the intents that the stored record lacks, so a retry after a conflict signs nothing twice.
            if (
                intents.some(
                    (intent) =>
                        !record.reviews[record.member.grant.member]?.operations.some(
                            (op) => op.operation.id === intent.id,
                        ),
                )
            ) {
                if (record.binding.permission === 'view') return fail('reviews-read-only', 403);
                const { result, packets } = this.reviews(record);
                for (const packet of packets)
                    for (const op of packet.operations) await result.apply(op, packet.grant.grant);
                const own = record.reviews[record.member.grant.member] ?? { grant: record.member, operations: [] };
                // An own operation passes the same checks as a received one before it enters the record.
                for (const operation of intents)
                    if (!own.operations.some((op) => op.operation.id === operation.id)) {
                        const signed = await signReview(operation, record.identity);
                        await result.apply(signed, record.member.grant);
                        own.operations.push(signed);
                    }
                record.reviews[record.member.grant.member] = own;
                this.queue(record, 'review', this.reviewBytes(record));
            }
            await this.save(record);
            this.failedLocal = false;
            return record.projection;
        }).catch((error) => {
            this.failedLocal = true;
            throw error;
        });
    }
    /**
     * Returns the envelope of the oldest queued packet, or `null` for an empty outbox. The first call seals the packet
     * and commits the exact ciphertext before it returns. A retry, also from another tab, therefore sends identical
     * bytes.
     */
    async prepare(): Promise<SignedCheckpoint | null> {
        return this.serial(async () => {
            const record = await this.latest(),
                pending = record.outbox[0];
            if (!pending) return null;
            if (pending.envelope) return pending.envelope;
            const b = record.binding;
            pending.envelope = await sealCheckpoint(
                {
                    v: 1,
                    collection: b.collection,
                    document: b.document,
                    member: b.member,
                    stream: pending.stream,
                    epoch: b.epoch,
                    sequence: pending.sequence,
                    operation: pending.operation,
                },
                decode(pending.bytes),
                decode(record.key),
                record.identity,
            );
            record.revision++;
            await this.repository.commit(record.revision - 1, record);
            this.record = structuredClone(record);
            return pending.envelope;
        });
    }
    /**
     * Removes an uploaded packet after the service confirms it. Both arguments come from the service response. The
     * digest must equal the local digest of the saved envelope. An operation that another tab acknowledged with the
     * same digest succeeds without a change.
     * @throws `SyncError` `acknowledgement-mismatch` (409) for an unknown operation or a different digest.
     */
    async acknowledge(operation: string, digest: string) {
        return this.serial(async () => {
            const record = await this.latest(),
                pending = record.outbox.find((p) => p.operation === operation);
            if (!pending && record.acknowledgements?.[operation] === digest) {
                this.record = structuredClone(record);
                return;
            }
            if (!pending?.envelope || (await checkpointDigest(pending.envelope)) !== digest)
                return fail('acknowledgement-mismatch', 409);
            // Keep the latest 64 digests, so another tab can acknowledge the same upload without a mismatch.
            record.acknowledgements = Object.fromEntries(
                [...Object.entries(record.acknowledgements ?? {}), [operation, digest]].slice(-64),
            );
            // Own operations already exist locally. Persist that with the ACK so
            // a reopened client does not download the same checkpoints again.
            const checkpoint = pending.envelope.checkpoint,
                cursor = `${checkpoint.member}/${checkpoint.stream}`;
            record.progress[cursor] = Math.max(record.progress[cursor] ?? 0, checkpoint.sequence);
            record.outbox = record.outbox.filter((p) => p.operation !== operation);
            record.revision++;
            await this.repository.commit(record.revision - 1, record);
            this.record = structuredClone(record);
        });
    }
    /** Number of queued packets in the stored record. */
    async pending() {
        return (await this.latest()).outbox.length;
    }
    /**
     * Loads the stored record into this session, for example after a commit in another tab.
     * @returns The projection. An outdated stored projection is rebuilt and saved first.
     */
    async synchronize() {
        return this.serial(async () => {
            const stored = await this.latest();
            this.replica.apply(decode(stored.body));
            const projection = await this.projected(stored);
            // Repair an older saved projection without changing signed history,
            // immutable pending packets, or network progress.
            if (projection !== stored.projection) {
                stored.projection = projection;
                stored.revision++;
                await this.repository.commit(stored.revision - 1, stored, true);
            }
            this.record = structuredClone(stored);
            return stored.projection;
        });
    }
    /**
     * Verifies and applies one downloaded checkpoint. State, projection, and progress commit together. The service and
     * the author control `envelope` and `author`. The checks run in this order:
     * 1. The checkpoint names this document and collection, at an epoch that this client reached.
     * 2. The grant of the author permits the stream and verifies under the owner grant from the record.
     * 3. The signature and the AES-GCM tag verify under the key of the author and the document key of the epoch.
     * 4. In a review checkpoint, each operation names the author as actor and passes `ReviewReplica.apply`.
     *
     * A checkpoint at or below the stored cursor changes nothing.
     * @returns The projection after the checkpoint.
     * @throws `RejectedPacket` when this one checkpoint fails steps 2 to 4. The caller skips it.
     * @throws `SyncError` `invalid-checkpoint-author` (403) or `key-epoch-unavailable` (409) when the checkpoint does
     *   not fit this connection.
     */
    async receive(envelope: SignedCheckpoint, author: SignedGrant) {
        return this.serial(async () => {
            const record = await this.latest(),
                c = envelope.checkpoint;
            if (
                c.document !== record.binding.document ||
                c.collection !== record.binding.collection ||
                c.epoch > record.binding.epoch
            )
                return fail('invalid-checkpoint-author', 403);
            // From here to the commit, a failure describes this one checkpoint, not the connection.
            if (!permitted(author.grant, c.document, c.stream) || !(await verifyGrant(author, record.owner.grant)))
                throw new RejectedPacket(new SyncError('invalid-checkpoint-author', 403));
            const cursor = `${c.member}/${c.stream}`;
            if ((record.progress[cursor] ?? 0) >= c.sequence) return record.projection;
            const key = record.keys?.[String(c.epoch)] ?? (c.epoch === record.binding.epoch ? record.key : undefined);
            if (!key) return fail('key-epoch-unavailable', 409);
            const replica = new BodyReplica(decode(record.body));
            try {
                try {
                    const bytes = await openCheckpoint(envelope, decode(key), author.grant);
                    if (c.stream === 'body' || c.stream === 'tree') {
                        replica.apply(bytes);
                        // Any editor can write `oldKeys`. Accept a well-formed key only for an epoch below the current
                        // one. A stored key is never replaced.
                        const historical = replica.doc.getMap('meta').get('oldKeys');
                        if (historical && typeof historical === 'object' && !Array.isArray(historical)) {
                            record.keys ??= { [String(record.binding.epoch)]: record.key };
                            for (const [epoch, value] of Object.entries(historical))
                                if (
                                    Number.isSafeInteger(Number(epoch)) &&
                                    Number(epoch) > 0 &&
                                    Number(epoch) < record.binding.epoch &&
                                    typeof value === 'string' &&
                                    /^[A-Za-z0-9_-]{43}$/.test(value)
                                )
                                    record.keys[epoch] ??= value;
                        }
                    } else {
                        const payload = JSON.parse(new TextDecoder().decode(bytes));
                        if (!Array.isArray(payload.operations) || payload.operations.length > 10000)
                            return fail('invalid-review-checkpoint');
                        // Only the document creator can supply the seed. The suffix must parse back to exactly those
                        // threads.
                        if (payload.seed !== undefined) {
                            if (
                                author.grant.member !== (record.creator ?? record.owner.grant.member) ||
                                !validSeed(payload.seed) ||
                                typeof payload.suffix !== 'string'
                            )
                                return fail('invalid-review-seed');
                            const parsed = splitComments('x' + payload.suffix);
                            if (payload.suffix && (parsed.body !== 'x' || !same(parsed.threads, payload.seed)))
                                return fail('invalid-review-suffix');
                            record.seed = payload.seed;
                            record.suffix = payload.suffix;
                        }
                        // Verify the complete payload before it replaces the stored operations of this author. The
                        // verifier holds only the seed and the operations of this author.
                        const verifier = new ReviewReplica(
                            record.binding.document,
                            record.seed,
                            record.owner.grant.participant,
                        );
                        for (const operation of payload.operations) {
                            if (operation?.operation?.actor !== author.grant.member)
                                return fail('review-author-mismatch', 403);
                            await verifier.apply(operation, author.grant);
                        }
                        record.reviews[author.grant.member] = {
                            grant: author,
                            operations: mergeOwnOperations(record, author, payload.operations),
                        };
                    }
                    record.body = encode(replica.checkpoint());
                    record.projection = await this.projected(record, replica);
                } catch (error) {
                    throw new RejectedPacket(error);
                }
                record.progress[cursor] = c.sequence;
                record.revision++;
                await this.repository.commit(record.revision - 1, record, true);
                this.replica.apply(replica.checkpoint());
                this.record = structuredClone(record);
                return record.projection;
            } finally {
                replica.destroy();
            }
        });
    }
    /**
     * Undoes or redoes the latest local body edit of this session and queues a body checkpoint.
     * @returns The committed projection.
     * @throws `SyncError` `body-read-only` (403) without edit permission.
     */
    async undo(redo = false) {
        return this.serial(async () => {
            const record = await this.latest();
            this.replica.apply(decode(record.body));
            if (!permitted(record.member.grant, record.binding.document, 'body')) return fail('body-read-only', 403);
            if (redo) this.replica.undo.redo();
            else this.replica.undo.undo();
            this.queue(record, 'body', this.replica.checkpoint());
            await this.save(record);
            return record.projection;
        });
    }
    /**
     * Tells the session that the editor displays `text` and has caught up with the committed record. Later local
     * changes are computed against this text and the committed reviews. It is synchronous and reads no storage.
     */
    present(text: string) {
        this.presented.apply(decode(this.record.body));
        this.presentedText = text;
        this.presentedReviews = structuredClone(this.record.reviews);
        const committed = new Set(
            Object.values(this.record.reviews).flatMap((packet) => packet.operations.map((op) => op.operation.id)),
        );
        this.localReviewIntents = this.localReviewIntents.filter((op) => !committed.has(op.id));
    }
    /**
     * Switches the document to a later key epoch. An older epoch, or the same epoch and key, changes nothing. The
     * earlier key stays in `keys` for older checkpoints. Each queued packet gets a fresh operation ID and sequence and
     * loses its envelope. The service accepts ciphertext of the current epoch only, and it binds an operation ID to one
     * ciphertext.
     * @param key Document key of `epoch` in base64url.
     * @param publish Also writes the earlier keys into the shared metadata and queues complete checkpoints under the
     *   fresh key. Receivers read keys of earlier epochs from that metadata.
     */
    async rekey(epoch: number, key: string, publish = false) {
        return this.serial(async () => {
            const record = await this.latest();
            if (epoch < record.binding.epoch) return;
            if (epoch === record.binding.epoch && key === record.key) return;
            const oldKeys = { ...(record.keys ?? {}), [String(record.binding.epoch)]: record.key };
            record.keys = { ...oldKeys, [String(epoch)]: key };
            record.key = key;
            record.binding.epoch = epoch;
            record.outbox = record.outbox.map((pending) => ({
                ...pending,
                operation: crypto.randomUUID(),
                sequence: record.nextSequence++,
                envelope: null,
            }));
            if (publish) {
                this.replica.apply(decode(record.body));
                this.replica.doc.getMap('meta').set('oldKeys', oldKeys);
                this.queue(record, this.folder() ? 'tree' : 'body', this.replica.checkpoint());
                if (!this.folder()) this.queue(record, 'review', this.reviewBytes(record));
            }
            await this.save(record);
        });
    }
    /**
     * Records that this participant reviewed the current body: a signed `ack` operation with the SHA-256 of the body
     * text. A view-only grant changes nothing.
     */
    async markReviewed() {
        return this.serial(async () => {
            const record = await this.latest();
            this.replica.apply(decode(record.body));
            if (record.member.grant.permission === 'view') return;
            const own = record.reviews[record.member.grant.member] ?? { grant: record.member, operations: [] };
            const previous = own.operations
                .filter((op) => op.operation.kind === 'ack')
                .slice(-1)
                .map((op) => op.operation.id);
            const operation: ReviewOperation = {
                v: 1,
                id: crypto.randomUUID(),
                document: record.binding.document,
                actor: record.member.grant.member,
                participant: record.member.grant.participant,
                previous,
                kind: 'ack',
                thread: `ack:${record.member.grant.participant}`,
                comment: null,
                value: await hash(new TextEncoder().encode(this.replica.text())),
            };
            own.operations.push(await signReview(operation, record.identity));
            record.reviews[record.member.grant.member] = own;
            this.queue(record, 'review', this.reviewBytes(record));
            await this.save(record);
        });
    }
    /**
     * Reverts a local review action. For each thread in `before`, a thread or comment that is missing in `after` gets
     * an `undo-delete` of the deletion that this member signed. A changed resolution returns to its earlier value.
     * @returns The committed projection.
     * @throws `SyncError` `review-undo-unavailable` (409) when this member signed no matching deletion.
     */
    async undoReview(before: Thread[], after: Thread[]) {
        return this.serial(async () => {
            const record = await this.latest(),
                own = record.reviews[record.member.grant.member] ?? { grant: record.member, operations: [] };
            const append = async (
                kind: ReviewOperation['kind'],
                thread: string,
                comment: string | null,
                value: ReviewOperation['value'],
            ) => {
                const previous = own.operations.findLast(
                    (op) => op.operation.thread === thread && op.operation.comment === comment,
                )?.operation.id;
                const operation: ReviewOperation = {
                    v: 1,
                    id: crypto.randomUUID(),
                    document: record.binding.document,
                    actor: record.member.grant.member,
                    participant: record.member.grant.participant,
                    previous: [
                        ...new Set([
                            ...(previous ? [previous] : []),
                            ...(kind === 'undo-delete' ? [value as string] : []),
                        ]),
                    ],
                    kind,
                    thread,
                    comment,
                    value: normalized(value),
                };
                const signed = await signReview(operation, record.identity);
                const { result, packets } = this.reviews(record);
                for (const packet of packets)
                    for (const op of packet.operations) await result.apply(op, packet.grant.grant);
                await result.apply(signed, record.member.grant);
                own.operations.push(signed);
                record.reviews[record.member.grant.member] = own;
            };
            const restore = async (thread: string, comment: string | null) => {
                const deleted = own.operations.findLast(
                    ({ operation }) =>
                        operation.thread === thread &&
                        operation.comment === comment &&
                        operation.kind === (comment === null ? 'delete-thread' : 'delete-comment'),
                );
                if (!deleted) return fail('review-undo-unavailable', 409);
                // Undo cancels our signed deletion. It never publishes another
                // author's comment text under our identity.
                await append('undo-delete', thread, comment, deleted.operation.id);
            };
            for (const thread of before) {
                const next = after.find((t) => t.id === thread.id);
                if (!next) {
                    await restore(thread.id, null);
                    continue;
                }
                if (!!thread.resolved !== !!next.resolved) await append('resolve', thread.id, null, !!thread.resolved);
                for (const comment of thread.comments)
                    if (!next.comments.some((c) => commentKey(c) === commentKey(comment)))
                        await restore(thread.id, commentKey(comment));
            }
            this.queue(record, 'review', this.reviewBytes(record));
            await this.save(record);
            return record.projection;
        });
    }
    /** Releases both Yjs documents. The stored record remains. */
    destroy() {
        this.replica.destroy();
        this.presented.destroy();
    }
}
