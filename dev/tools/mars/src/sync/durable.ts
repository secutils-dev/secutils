// Durable body replica and outbox, independent of UI and HTTP. Repository.commit must
// atomically save projection, replica, progress, and outbox (db.commitReplica does this).
// This engine covers one body stream at one key epoch, without reviews. `Session` in session.ts applies the
// same rules to connected documents with reviews, manifests, and key rotation.
// Durability order: the plain local change, then the exact ciphertext, then the upload, then the acknowledgement.
import { BodyReplica, type SourceChange } from './replica.ts';
import { decode, encode, fail, permitted, type Grant, type SignedCheckpoint, type SyncBinding } from './protocol.ts';
import { checkpointDigest, openCheckpoint, sealCheckpoint, type Identity } from './crypto.ts';

/**
 * One queued body upload. `bytes` is the plaintext Yjs state in base64url. `envelope` is `null` until `prepare` seals
 * the packet, and never changes afterwards.
 */
export interface PendingBody {
    operation: string;
    sequence: number;
    bytes: string;
    envelope: SignedCheckpoint | null;
}
/**
 * Stored state of one body replica. `revision` increments with each commit. `body` is the Yjs state in base64url and
 * `projection` is its text. `progress` holds the highest applied sequence for each `<member>/<stream>`.
 */
export interface DurableRecord {
    v: 1;
    revision: number;
    binding: SyncBinding;
    body: string;
    projection: string;
    nextSequence: number;
    outbox: PendingBody[];
    progress: Record<string, number>;
}
/**
 * Storage of one record. `commit` must be atomic and must reject when the stored revision differs from `expected`.
 * `null` expects no stored record.
 */
export interface ReplicaRepository {
    get(): Promise<DurableRecord | undefined>;
    commit(expected: number | null, value: DurableRecord): Promise<void>;
}
/**
 * A body replica that commits each change before the network or the in-memory document can see it. Operations run one
 * at a time. The instance holds a copy of the document key and zeroes it in `destroy`.
 */
export class DurableBody {
    readonly body: BodyReplica;
    readonly repository: ReplicaRepository;
    readonly identity: Identity;
    readonly key: Uint8Array<ArrayBuffer>;
    readonly binding: SyncBinding;
    private queue: Promise<unknown> = Promise.resolve();
    constructor(
        record: DurableRecord,
        repository: ReplicaRepository,
        identity: Identity,
        key: Uint8Array<ArrayBuffer>,
    ) {
        this.body = new BodyReplica(decode(record.body));
        this.repository = repository;
        this.identity = identity;
        this.key = new Uint8Array(key);
        this.binding = { ...record.binding };
    }
    /**
     * Creates the replica of a document creator from `text` and queues its first checkpoint with sequence 1.
     * @throws `SyncError` `creator-required` (403) for a binding without owner permission, or `binding-already-exists`
     *   (409) when the repository holds a record.
     */
    static async create(
        binding: SyncBinding,
        text: string,
        repository: ReplicaRepository,
        identity: Identity,
        key: Uint8Array<ArrayBuffer>,
    ) {
        if (binding.permission !== 'owner') return fail('creator-required', 403);
        if (await repository.get()) return fail('binding-already-exists', 409);
        const body = new BodyReplica();
        body.seed(text);
        const record: DurableRecord = {
            v: 1,
            revision: 1,
            binding,
            body: encode(body.checkpoint()),
            projection: text,
            nextSequence: 2,
            outbox: [{ operation: crypto.randomUUID(), sequence: 1, bytes: encode(body.checkpoint()), envelope: null }],
            progress: {},
        };
        body.destroy();
        await repository.commit(null, record);
        return new DurableBody(record, repository, identity, key);
    }
    /**
     * Creates the replica of a joiner from the decrypted checkpoint of the creator. Its outbox starts empty.
     * @throws `SyncError` `binding-already-exists` (409) when the repository holds a record.
     */
    static async join(
        binding: SyncBinding,
        checkpoint: Uint8Array<ArrayBuffer>,
        repository: ReplicaRepository,
        identity: Identity,
        key: Uint8Array<ArrayBuffer>,
    ) {
        if (await repository.get()) return fail('binding-already-exists', 409);
        // A joiner adopts the creator's CRDT identity. An independent local text seed
        // must remain separate until the user makes an explicit merge decision.
        const body = new BodyReplica(checkpoint);
        const record: DurableRecord = {
            v: 1,
            revision: 1,
            binding,
            body: encode(body.checkpoint()),
            projection: body.text(),
            nextSequence: 1,
            outbox: [],
            progress: {},
        };
        body.destroy();
        await repository.commit(null, record);
        return new DurableBody(record, repository, identity, key);
    }
    /** Runs `run` after all earlier operations. A failed operation does not block the later ones. */
    private serial<T>(run: () => Promise<T>): Promise<T> {
        const task = this.queue.then(run);
        this.queue = task.catch(() => {});
        return task;
    }
    /** Reads the stored record. Fails with `connection-detached` (409) when it is missing or has another binding. */
    private async record() {
        const record = await this.repository.get();
        if (
            !record ||
            record.binding.collection !== this.binding.collection ||
            record.binding.document !== this.binding.document ||
            record.binding.member !== this.binding.member
        )
            return fail('connection-detached', 409);
        return record;
    }
    /** Number of queued uploads in the stored record. */
    async pending() {
        return (await this.record()).outbox.length;
    }
    /**
     * Builds the next record after a local change. The complete current state replaces the payload of an unsealed tail
     * packet, or becomes a fresh packet.
     */
    private intent(record: DurableRecord) {
        const bytes = encode(this.body.checkpoint()),
            pending = structuredClone(record.outbox),
            tail = pending.at(-1);
        if (tail && !tail.envelope) tail.bytes = bytes;
        else pending.push({ operation: crypto.randomUUID(), sequence: record.nextSequence++, bytes, envelope: null });
        return { ...record, revision: record.revision + 1, body: bytes, projection: this.body.text(), outbox: pending };
    }
    /**
     * Applies a local edit and commits it together with its outbox entry.
     * @returns The committed record.
     * @throws `SyncError` `body-read-only` (403), or `replica-changed` (409) when storage differs from this instance.
     */
    edit(change: SourceChange) {
        return this.serial(async () => {
            if (!['edit', 'owner'].includes(this.binding.permission)) return fail('body-read-only', 403);
            const record = await this.record();
            // The host supplies a range from this replica's current projection. It must
            // not silently convert a stale, independent editor seed into a replacement.
            if (record.body !== encode(this.body.checkpoint())) return fail('replica-changed', 409);
            this.body.edit(change);
            const next = this.intent(record);
            // Plain local operations commit BEFORE encryption or any upload.
            await this.repository.commit(record.revision, next);
            return next;
        });
    }
    /**
     * Undoes or redoes the latest local edit and commits the result. An empty stack returns the stored record. It fails
     * in the same cases as `edit`.
     */
    undo(redo = false) {
        return this.serial(async () => {
            if (!['edit', 'owner'].includes(this.binding.permission)) return fail('body-read-only', 403);
            const record = await this.record();
            if (record.body !== encode(this.body.checkpoint())) return fail('replica-changed', 409);
            if (!(redo ? this.body.undo.redoStack : this.body.undo.undoStack).length) return record;
            if (redo) this.body.undo.redo();
            else this.body.undo.undo();
            const next = this.intent(record);
            await this.repository.commit(record.revision, next);
            return next;
        });
    }
    /**
     * Returns the envelope of the oldest queued packet, or `null` for an empty outbox. The first call seals the packet
     * and commits the exact ciphertext before it returns.
     */
    prepare() {
        return this.serial(async () => {
            const record = await this.record(),
                first = record.outbox[0];
            if (!first) return null;
            if (first.envelope) return structuredClone(first.envelope);
            const b = record.binding;
            const envelope = await sealCheckpoint(
                {
                    v: 1,
                    collection: b.collection,
                    document: b.document,
                    member: b.member,
                    stream: 'body',
                    epoch: b.epoch,
                    sequence: first.sequence,
                    operation: first.operation,
                },
                decode(first.bytes),
                this.key,
                this.identity,
            );
            const next = {
                ...record,
                revision: record.revision + 1,
                outbox: record.outbox.map((entry, index) => (index ? entry : { ...entry, envelope })),
            };
            // Persist the exact nonce/ciphertext before HTTP can see it. A retry never encrypts again.
            await this.repository.commit(record.revision, next);
            return envelope;
        });
    }
    /**
     * Removes a packet after the service confirms it. The digest comes from the service and must equal the local digest
     * of the saved envelope.
     * @throws `SyncError` `acknowledgement-mismatch` (409) for an unknown operation or a different digest.
     */
    acknowledge(operation: string, digest: string) {
        return this.serial(async () => {
            const record = await this.record(),
                pending = record.outbox.find((entry) => entry.operation === operation);
            if (!pending?.envelope || (await checkpointDigest(pending.envelope)) !== digest)
                return fail('acknowledgement-mismatch', 409);
            await this.repository.commit(record.revision, {
                ...record,
                revision: record.revision + 1,
                outbox: record.outbox.filter((entry) => entry.operation !== operation),
            });
        });
    }
    /**
     * Verifies, decrypts, and applies a downloaded body checkpoint. State and progress commit together. `envelope` is
     * untrusted. `author` must be a grant that the caller verified against the owner key. The checkpoint must name the
     * bound collection and document, the current epoch, and the body stream, and the author needs edit permission. A
     * checkpoint at or below the stored cursor changes nothing.
     * @returns The projection after the checkpoint.
     * @throws `SyncError` `unauthorized-body-checkpoint` (403), or an error of `openCheckpoint`.
     */
    receive(envelope: SignedCheckpoint, author: Grant) {
        return this.serial(async () => {
            const record = await this.record(),
                c = envelope.checkpoint,
                b = record.binding;
            if (
                c.collection !== b.collection ||
                c.document !== b.document ||
                c.epoch !== b.epoch ||
                c.stream !== 'body' ||
                !permitted(author, c.document, c.stream)
            )
                return fail('unauthorized-body-checkpoint', 403);
            const stream = `${c.member}/${c.stream}`;
            if ((record.progress[stream] ?? 0) >= c.sequence) return record.projection;
            const bytes = await openCheckpoint(envelope, this.key, author),
                prepared = new BodyReplica(decode(record.body));
            try {
                prepared.apply(bytes);
                const next = {
                    ...record,
                    revision: record.revision + 1,
                    body: encode(prepared.checkpoint()),
                    projection: prepared.text(),
                    progress: { ...record.progress, [stream]: c.sequence },
                };
                await this.repository.commit(record.revision, next);
                // Incoming state changes only after both its projection and progress are durable.
                this.body.apply(bytes);
                return next.projection;
            } finally {
                prepared.destroy();
            }
        });
    }
    /** Releases the Yjs document and zeroes the key copy. */
    destroy() {
        this.body.destroy();
        this.key.fill(0);
    }
}
