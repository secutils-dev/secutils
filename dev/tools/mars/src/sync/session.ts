// A connected document: body CRDT, author-separated reviews, and a durable outbox.
// Plain local state commits first. Ciphertexts remain immutable until acknowledgement.
import * as Y from 'yjs';
import { BodyReplica, sourceChange } from './replica.ts';
import { ReviewReplica, signReview, type SignedReview, type ReviewOperation } from './reviews.ts';
import { commentKey, joinComments, splitComments, type Thread } from '../share/merge.ts';
import { decode, encode, fail, jsonBytes, canonical, permitted, RejectedPacket, SyncError, type SyncBinding, type SignedCheckpoint, type SignedGrant, type Stream } from './protocol.ts';
import { checkpointDigest, hash, openCheckpoint, sealCheckpoint, verifyGrant, type Identity } from './crypto.ts';

export interface PendingPacket { operation: string; sequence: number; stream: Stream; bytes: string; envelope: SignedCheckpoint | null }
export interface SessionRecord {
    v: 1; kind: 'connected'; revision: number; binding: SyncBinding; identity: Identity; token: string; key: string;
    owner: SignedGrant; member: SignedGrant; body: string; projection: string; seed: Thread[]; suffix: string; creator?: string;
    reviews: Record<string, { grant: SignedGrant; operations: SignedReview[] }>;
    outbox: PendingPacket[]; nextSequence: number; progress: Record<string, number>;
    ended?: boolean;
    keys?: Record<string, string>;
    acknowledgements?: Record<string, string>;
}
export interface SessionRepository { get(): Promise<SessionRecord | undefined>; commit(expected: number | null, value: SessionRecord, remote?: boolean): Promise<void> }
export interface ManifestNode { id: string; kind: 'doc' | 'folder'; parent: string | null; name: string; order: number; key?: string; origin?: string; epoch?: number }
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
// Incoming replicas stay private until their entire folder can be committed.
export function stagedSession(initial: SessionRecord) {
    let record = structuredClone(initial);
    return new Session(record, { get: async () => structuredClone(record), commit: async (expected, value) => {
        if (record.revision !== expected) throw new Error('Staged replica changed');
        record = structuredClone(value);
    } });
}
const normalized = <T>(value: T): T => JSON.parse(JSON.stringify(value));
const same = (a: unknown, b: unknown) => canonical(normalized(a)) === canonical(normalized(b));
const validSeed = (value: unknown): value is Thread[] => Array.isArray(value) && value.length <= 1000 && value.every((t) => t && typeof t.id === 'string'
    && typeof t.quote === 'string' && t.quote.length <= 8192 && Array.isArray(t.comments) && t.comments.length <= 1000
    && t.comments.every((c: unknown) => !!c && typeof (c as { body?: unknown }).body === 'string'));

export class Session {
    readonly replica: BodyReplica;
    readonly presented: BodyReplica;
    readonly repository: SessionRepository;
    record: SessionRecord;
    private tasks: Promise<unknown> = Promise.resolve();
    private presentedText: string;
    private presentedReviews: SessionRecord['reviews'];
    private localReviewIntents: ReviewOperation[] = [];
    private failedLocal = false;
    constructor(record: SessionRecord, repository: SessionRepository) {
        this.record = structuredClone(record); this.repository = repository; this.replica = new BodyReplica(decode(record.body));
        this.presented = new BodyReplica(decode(record.body)); this.presentedText = record.projection;
        this.presentedReviews = structuredClone(record.reviews);
    }
    static seed(binding: SyncBinding, text: string, who: Identity, token: string, key: string, owner: SignedGrant, name: string, member = owner): SessionRecord {
        const parsed = splitComments(text), replica = new BodyReplica(); replica.seed(parsed.body);
        replica.doc.getMap('meta').set('name', name);
        const value: SessionRecord = { v: 1, kind: 'connected', revision: 1, binding, identity: who, token, key, owner, member, creator: member.grant.member,
            body: encode(replica.checkpoint()), projection: text, seed: parsed.threads, suffix: text.slice(parsed.body.length),
            reviews: {}, outbox: [], nextSequence: 1, progress: {} };
        replica.destroy(); return value;
    }
    private serial<T>(run: () => Promise<T>) {
        const task = this.tasks.then(async () => {
            for (let attempt = 0; ; attempt++) {
                try { return await run(); }
                catch (error) { if ((error as Error)?.name !== 'ReplicaConflict' || attempt >= 4) throw error; }
            }
        });
        this.tasks = task.catch(() => {}); return task;
    }
    private async latest() {
        const stored = await this.repository.get();
        if (!stored || stored.ended || stored.binding.member !== this.record.binding.member) return fail('connection-ended', 409);
        return stored;
    }
    private reviews(record = this.record) {
        const result = new ReviewReplica(record.binding.document, record.seed, record.owner.grant.participant);
        return { result, packets: Object.values(record.reviews) };
    }
    private async projected(record: SessionRecord, replica = this.replica) {
        const { result, packets } = this.reviews(record);
        // Each outer checkpoint carries only its actual author's operations. Forwarding
        // another author's operation cannot bypass revocation or claim that author's identity.
        for (const packet of packets) for (const op of packet.operations) await result.apply(op, packet.grant.grant);
        const body = replica.text(), threads = result.threads();
        if (same(threads, record.seed)) {
            if (record.suffix) return body + record.suffix;
            if (!splitComments(body).threads.length) return body;
        }
        const joined = threads.length ? joinComments(body, threads) : `${body}${body.endsWith('\n') ? '' : '\n'}\n<!-- su-comments\n[]\n-->\n`;
        return body.includes('\r\n') ? joined.replace(/\r?\n/g, '\r\n') : joined;
    }
    private queue(record: SessionRecord, stream: Stream, bytes: Uint8Array) {
        const encoded = encode(bytes), pending = structuredClone(record.outbox), tail = pending.findLast((p) => p.stream === stream && !p.envelope);
        if (tail) tail.bytes = encoded;
        else pending.push({ operation: crypto.randomUUID(), sequence: record.nextSequence++, stream, bytes: encoded, envelope: null });
        record.outbox = pending;
    }
    private reviewBytes(record: SessionRecord) {
        return jsonBytes({ operations: record.reviews[record.member.grant.member]?.operations ?? [],
            ...(record.member.grant.member === (record.creator ?? record.owner.grant.member) ? { seed: record.seed, suffix: record.suffix } : {}) });
    }
    async initial() {
        return this.serial(async () => {
            const record = await this.latest();
            this.queue(record, this.folder() ? 'tree' : 'body', this.replica.checkpoint());
            if (!this.folder()) this.queue(record, 'review', this.reviewBytes(record));
            await this.save(record);
        });
    }
    folder() { return this.replica.doc.getMap('meta').get('kind') === 'folder'; }
    metadata() { return this.replica.doc.getMap('meta').toJSON() as { name?: string; readingWidth?: number | 'full'; kind?: string; origin?: string }; }
    manifest(): ManifestNode[] {
        const entries = this.replica.doc.getMap<Y.Map<unknown>>('nodes');
        return [...entries.entries()].flatMap(([id, value]) => value instanceof Y.Map ? [{ id, ...value.toJSON() } as ManifestNode] : []);
    }
    async editManifest(values: ManifestNode[]) {
        return this.serial(async () => {
            const record = await this.latest(); this.replica.apply(decode(record.body));
            this.replica.doc.transact(() => {
                const entries = this.replica.doc.getMap<Y.Map<unknown>>('nodes');
                for (const value of values) {
                    let item = entries.get(value.id); if (!(item instanceof Y.Map)) { item = new Y.Map(); entries.set(value.id, item); }
                    for (const [key, field] of Object.entries(value)) if (key !== 'id' && field !== undefined && item.get(key) !== field) item.set(key, field);
                }
            }, this.replica.local);
            this.queue(record, 'tree', this.replica.checkpoint()); await this.save(record);
        });
    }
    async metadataChanged(values: Record<string, string | number | null>) {
        return this.serial(async () => {
            const record = await this.latest(); this.replica.apply(decode(record.body));
            if (!permitted(record.member.grant, record.binding.document, this.folder() ? 'tree' : 'body')) return fail('metadata-read-only', 403);
            this.replica.doc.transact(() => { const meta = this.replica.doc.getMap('meta'); for (const [key, value] of Object.entries(values)) meta.set(key, value); }, this.replica.local);
            this.queue(record, this.folder() ? 'tree' : 'body', this.replica.checkpoint()); await this.save(record);
        });
    }
    private async save(record: SessionRecord) {
        record.body = encode(this.replica.checkpoint()); record.projection = await this.projected(record); record.revision++;
        await this.repository.commit(record.revision - 1, record); this.record = structuredClone(record);
    }
    async change(before: string, after: string) {
        // Stage against the actual editor's CRDT synchronously. A remote transaction can
        // commit while IndexedDB is busy without making the next keystroke replace it.
        const old = splitComments(this.failedLocal ? this.record.projection : this.presentedText), next = splitComments(after);
        const change = sourceChange(splitComments(this.presentedText).body, next.body);
        if (change) {
            if (!permitted(this.record.member.grant, this.record.binding.document, 'body')) return fail('body-read-only', 403);
            this.presented.edit(change);
        }
        const staged = this.presented.checkpoint(); this.presentedText = after;
        // Parents come from what this editor actually saw, not a newer unseen peer
        // version that happened to reach IndexedDB while the user typed.
        const causal = new ReviewReplica(this.record.binding.document, this.record.seed, this.record.owner.grant.participant);
        for (const packet of Object.values(this.presentedReviews)) for (const op of packet.operations) causal.operations.set(op.operation.id, op);
        for (const operation of this.localReviewIntents) causal.operations.set(operation.id, { operation, signature: '' });
        const appendIntent = (kind: ReviewOperation['kind'], thread: string, comment: string | null, value: ReviewOperation['value']) => {
            const previous = causal.heads(thread, comment, kind).map((p) => p.operation.id);
            const lastByActor = new Map<string, string>();
            // A reply follows the latest operation from every author it actually saw
            // in this thread, not only the replying author's previous operation.
            for (const { operation } of causal.operations.values()) if (operation.thread === thread) lastByActor.set(operation.actor, operation.id);
            previous.push(...lastByActor.values());
            const operation: ReviewOperation = { v: 1, id: crypto.randomUUID(), document: this.record.binding.document,
                actor: this.record.member.grant.member, participant: this.record.member.grant.participant,
                previous: [...new Set(previous)], kind, thread, comment, value: normalized(value) };
            this.localReviewIntents.push(operation); causal.operations.set(operation.id, { operation, signature: '' });
        };
        if (!same(old.threads, next.threads)) {
            for (const thread of next.threads) {
                const previous = old.threads.find((t) => t.id === thread.id);
                const fields = (t: Thread) => ({ ...t, comments: [], resolved: undefined });
                if (!previous || !same(fields(previous), fields(thread))) appendIntent('thread', thread.id, null, fields(thread));
                if (!!previous?.resolved !== !!thread.resolved) appendIntent('resolve', thread.id, null, !!thread.resolved);
                for (const comment of thread.comments) {
                    const key = commentKey(comment), oldComment = previous?.comments.find((c) => commentKey(c) === key);
                    if (!oldComment || !same(oldComment, comment)) appendIntent('comment', thread.id, key, { ...comment, id: key });
                }
                for (const comment of previous?.comments ?? []) if (!thread.comments.some((c) => commentKey(c) === commentKey(comment))) appendIntent('delete-comment', thread.id, commentKey(comment), null);
            }
            for (const thread of old.threads) if (!next.threads.some((t) => t.id === thread.id)) appendIntent('delete-thread', thread.id, null, null);
        }
        const intents = [...this.localReviewIntents];
        return this.serial(async () => {
            const record = await this.latest();
            this.replica.apply(decode(record.body));
            if (change) {
                if (!permitted(record.member.grant, record.binding.document, 'body')) return fail('body-read-only', 403);
            }
            this.replica.applyLocal(staged);
            if (change || this.failedLocal) this.queue(record, 'body', this.replica.checkpoint());
            if (intents.some((intent) => !record.reviews[record.member.grant.member]?.operations.some((op) => op.operation.id === intent.id))) {
                if (record.binding.permission === 'view') return fail('reviews-read-only', 403);
                const { result, packets } = this.reviews(record);
                for (const packet of packets) for (const op of packet.operations) await result.apply(op, packet.grant.grant);
                const own = record.reviews[record.member.grant.member] ?? { grant: record.member, operations: [] };
                for (const operation of intents) if (!own.operations.some((op) => op.operation.id === operation.id)) {
                    const signed = await signReview(operation, record.identity); await result.apply(signed, record.member.grant); own.operations.push(signed);
                }
                record.reviews[record.member.grant.member] = own; this.queue(record, 'review', this.reviewBytes(record));
            }
            await this.save(record); this.failedLocal = false; return record.projection;
        }).catch((error) => { this.failedLocal = true; throw error; });
    }
    async prepare(): Promise<SignedCheckpoint | null> {
        return this.serial(async () => {
            const record = await this.latest(), pending = record.outbox[0]; if (!pending) return null;
            if (pending.envelope) return pending.envelope;
            const b = record.binding;
            pending.envelope = await sealCheckpoint({ v: 1, collection: b.collection, document: b.document, member: b.member,
                stream: pending.stream, epoch: b.epoch, sequence: pending.sequence, operation: pending.operation }, decode(pending.bytes), decode(record.key), record.identity);
            record.revision++; await this.repository.commit(record.revision - 1, record); this.record = structuredClone(record); return pending.envelope;
        });
    }
    async acknowledge(operation: string, digest: string) {
        return this.serial(async () => {
            const record = await this.latest(), pending = record.outbox.find((p) => p.operation === operation);
            if (!pending && record.acknowledgements?.[operation] === digest) { this.record = structuredClone(record); return; }
            if (!pending?.envelope || await checkpointDigest(pending.envelope) !== digest) return fail('acknowledgement-mismatch', 409);
            record.acknowledgements = Object.fromEntries([...Object.entries(record.acknowledgements ?? {}), [operation, digest]].slice(-64));
            // Own operations already exist locally. Persist that with the ACK so
            // a reopened client does not download the same checkpoints again.
            const checkpoint = pending.envelope.checkpoint, cursor = `${checkpoint.member}/${checkpoint.stream}`;
            record.progress[cursor] = Math.max(record.progress[cursor] ?? 0, checkpoint.sequence);
            record.outbox = record.outbox.filter((p) => p.operation !== operation); record.revision++;
            await this.repository.commit(record.revision - 1, record); this.record = structuredClone(record);
        });
    }
    async pending() { return (await this.latest()).outbox.length; }
    async synchronize() {
        return this.serial(async () => {
            const stored = await this.latest();
            this.replica.apply(decode(stored.body));
            const projection = await this.projected(stored);
            // Repair an older saved projection without changing signed history,
            // immutable pending packets, or network progress.
            if (projection !== stored.projection) {
                stored.projection = projection; stored.revision++;
                await this.repository.commit(stored.revision - 1, stored, true);
            }
            this.record = structuredClone(stored);
            return stored.projection;
        });
    }
    async receive(envelope: SignedCheckpoint, author: SignedGrant) {
        return this.serial(async () => {
            const record = await this.latest(), c = envelope.checkpoint;
            if (c.document !== record.binding.document || c.collection !== record.binding.collection || c.epoch > record.binding.epoch) return fail('invalid-checkpoint-author', 403);
            // From here to the commit, a failure describes this one checkpoint, not the connection.
            if (!permitted(author.grant, c.document, c.stream) || !await verifyGrant(author, record.owner.grant)) throw new RejectedPacket(new SyncError('invalid-checkpoint-author', 403));
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
                    const historical = replica.doc.getMap('meta').get('oldKeys');
                    if (historical && typeof historical === 'object' && !Array.isArray(historical)) {
                        record.keys ??= { [String(record.binding.epoch)]: record.key };
                        for (const [epoch, value] of Object.entries(historical)) if (Number.isSafeInteger(Number(epoch)) && Number(epoch) > 0
                            && Number(epoch) < record.binding.epoch && typeof value === 'string' && /^[A-Za-z0-9_-]{43}$/.test(value)) record.keys[epoch] ??= value;
                    }
                }
                else {
                    const payload = JSON.parse(new TextDecoder().decode(bytes));
                    if (!Array.isArray(payload.operations) || payload.operations.length > 10000) return fail('invalid-review-checkpoint');
                    if (payload.seed !== undefined) {
                        if (author.grant.member !== (record.creator ?? record.owner.grant.member) || !validSeed(payload.seed) || typeof payload.suffix !== 'string') return fail('invalid-review-seed');
                        const parsed = splitComments('x' + payload.suffix);
                        if (payload.suffix && (parsed.body !== 'x' || !same(parsed.threads, payload.seed))) return fail('invalid-review-suffix');
                        record.seed = payload.seed; record.suffix = payload.suffix;
                    }
                    const verifier = new ReviewReplica(record.binding.document, record.seed, record.owner.grant.participant);
                    for (const operation of payload.operations) {
                        if (operation?.operation?.actor !== author.grant.member) return fail('review-author-mismatch', 403);
                        await verifier.apply(operation, author.grant);
                    }
                    record.reviews[author.grant.member] = { grant: author, operations: mergeOwnOperations(record, author, payload.operations) };
                }
                record.body = encode(replica.checkpoint()); record.projection = await this.projected(record, replica);
                } catch (error) { throw new RejectedPacket(error); }
                record.progress[cursor] = c.sequence; record.revision++;
                await this.repository.commit(record.revision - 1, record, true);
                this.replica.apply(replica.checkpoint()); this.record = structuredClone(record); return record.projection;
            } finally { replica.destroy(); }
        });
    }
    async undo(redo = false) {
        return this.serial(async () => {
            const record = await this.latest(); this.replica.apply(decode(record.body));
            if (!permitted(record.member.grant, record.binding.document, 'body')) return fail('body-read-only', 403);
            if (redo) this.replica.undo.redo(); else this.replica.undo.undo();
            this.queue(record, 'body', this.replica.checkpoint()); await this.save(record); return record.projection;
        });
    }
    present(text: string) {
        this.presented.apply(decode(this.record.body)); this.presentedText = text;
        this.presentedReviews = structuredClone(this.record.reviews);
        const committed = new Set(Object.values(this.record.reviews).flatMap((packet) => packet.operations.map((op) => op.operation.id)));
        this.localReviewIntents = this.localReviewIntents.filter((op) => !committed.has(op.id));
    }
    async rekey(epoch: number, key: string, publish = false) {
        return this.serial(async () => {
            const record = await this.latest();
            if (epoch < record.binding.epoch) return;
            if (epoch === record.binding.epoch && key === record.key) return;
            const oldKeys = { ...(record.keys ?? {}), [String(record.binding.epoch)]: record.key };
            record.keys = { ...oldKeys, [String(epoch)]: key }; record.key = key; record.binding.epoch = epoch;
            record.outbox = record.outbox.map((pending) => ({ ...pending, operation: crypto.randomUUID(), sequence: record.nextSequence++, envelope: null }));
            if (publish) {
                this.replica.apply(decode(record.body));
                this.replica.doc.getMap('meta').set('oldKeys', oldKeys);
                this.queue(record, this.folder() ? 'tree' : 'body', this.replica.checkpoint());
                if (!this.folder()) this.queue(record, 'review', this.reviewBytes(record));
            }
            await this.save(record);
        });
    }
    async markReviewed() {
        return this.serial(async () => {
            const record = await this.latest();
            this.replica.apply(decode(record.body));
            if (record.member.grant.permission === 'view') return;
            const own = record.reviews[record.member.grant.member] ?? { grant: record.member, operations: [] };
            const previous = own.operations.filter((op) => op.operation.kind === 'ack').slice(-1).map((op) => op.operation.id);
            const operation: ReviewOperation = { v: 1, id: crypto.randomUUID(), document: record.binding.document, actor: record.member.grant.member,
                participant: record.member.grant.participant, previous, kind: 'ack', thread: `ack:${record.member.grant.participant}`, comment: null, value: await hash(new TextEncoder().encode(this.replica.text())) };
            own.operations.push(await signReview(operation, record.identity)); record.reviews[record.member.grant.member] = own;
            this.queue(record, 'review', this.reviewBytes(record)); await this.save(record);
        });
    }
    async undoReview(before: Thread[], after: Thread[]) {
        return this.serial(async () => {
            const record = await this.latest(), own = record.reviews[record.member.grant.member] ?? { grant: record.member, operations: [] };
            const append = async (kind: ReviewOperation['kind'], thread: string, comment: string | null, value: ReviewOperation['value']) => {
                const previous = own.operations.findLast((op) => op.operation.thread === thread && op.operation.comment === comment)?.operation.id;
                const operation: ReviewOperation = { v: 1, id: crypto.randomUUID(), document: record.binding.document,
                    actor: record.member.grant.member, participant: record.member.grant.participant,
                    previous: [...new Set([...(previous ? [previous] : []), ...(kind === 'undo-delete' ? [value as string] : [])])], kind, thread, comment, value: normalized(value) };
                const signed = await signReview(operation, record.identity);
                const { result, packets } = this.reviews(record); for (const packet of packets) for (const op of packet.operations) await result.apply(op, packet.grant.grant);
                await result.apply(signed, record.member.grant); own.operations.push(signed); record.reviews[record.member.grant.member] = own;
            };
            const restore = async (thread: string, comment: string | null) => {
                const deleted = own.operations.findLast(({ operation }) => operation.thread === thread && operation.comment === comment
                    && operation.kind === (comment === null ? 'delete-thread' : 'delete-comment'));
                if (!deleted) return fail('review-undo-unavailable', 409);
                // Undo cancels our signed deletion. It never publishes another
                // author's comment text under our identity.
                await append('undo-delete', thread, comment, deleted.operation.id);
            };
            for (const thread of before) {
                const next = after.find((t) => t.id === thread.id);
                if (!next) { await restore(thread.id, null); continue; }
                if (!!thread.resolved !== !!next.resolved) await append('resolve', thread.id, null, !!thread.resolved);
                for (const comment of thread.comments) if (!next.comments.some((c) => commentKey(c) === commentKey(comment))) await restore(thread.id, commentKey(comment));
            }
            this.queue(record, 'review', this.reviewBytes(record)); await this.save(record); return record.projection;
        });
    }
    destroy() { this.replica.destroy(); this.presented.destroy(); }
}
