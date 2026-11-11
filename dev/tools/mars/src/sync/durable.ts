// Durable body replica and outbox, independent of UI and HTTP. Repository.commit must
// atomically save projection, replica, progress, and outbox (db.commitReplica does this).
import { BodyReplica, type SourceChange } from './replica.ts';
import { decode, encode, fail, permitted, type Grant, type SignedCheckpoint, type SyncBinding } from './protocol.ts';
import { checkpointDigest, openCheckpoint, sealCheckpoint, type Identity } from './crypto.ts';

export interface PendingBody {
    operation: string;
    sequence: number;
    bytes: string;
    envelope: SignedCheckpoint | null;
}
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
export interface ReplicaRepository {
    get(): Promise<DurableRecord | undefined>;
    commit(expected: number | null, value: DurableRecord): Promise<void>;
}
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
    private serial<T>(run: () => Promise<T>): Promise<T> {
        const task = this.queue.then(run);
        this.queue = task.catch(() => {});
        return task;
    }
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
    async pending() {
        return (await this.record()).outbox.length;
    }
    private intent(record: DurableRecord) {
        const bytes = encode(this.body.checkpoint()),
            pending = structuredClone(record.outbox),
            tail = pending.at(-1);
        if (tail && !tail.envelope) tail.bytes = bytes;
        else pending.push({ operation: crypto.randomUUID(), sequence: record.nextSequence++, bytes, envelope: null });
        return { ...record, revision: record.revision + 1, body: bytes, projection: this.body.text(), outbox: pending };
    }
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
    destroy() {
        this.body.destroy();
        this.key.fill(0);
    }
}
