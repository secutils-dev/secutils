// Review replication is NOT a Yjs blob. Only these typed, author-bound operations can
// change reviews. Concurrent comment versions remain visible rather than using timestamps.
import { commentKey, joinComments, splitComments, type Comment, type Thread } from '../share/merge.ts';
import { canonical, fail, validId, type Grant } from './protocol.ts';
import { sign, verify, type Identity } from './crypto.ts';

export interface ReviewOperation {
    v: 1; id: string; document: string; actor: string; participant: string;
    previous: string[]; kind: 'thread' | 'comment' | 'resolve' | 'delete-comment' | 'delete-thread' | 'undo-delete' | 'ack';
    thread: string; comment: string | null; value: Thread | Comment | boolean | string | null;
}
export interface SignedReview { operation: ReviewOperation; signature: string }
export const signReview = async (operation: ReviewOperation, who: Identity): Promise<SignedReview> => ({ operation, signature: await sign(operation, who) });
const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const shortString = (value: unknown, length: number): value is string => typeof value === 'string' && value.length <= length;
const validComment = (value: unknown): value is Comment => object(value) && shortString(value.body, 65536)
    && Object.keys(value).every((key) => ['id', 'author', 'at', 'body', 'edited'].includes(key))
    && [value.id, value.author, value.at, value.edited].every((field) => field === undefined || shortString(field, 256));
const validThread = (value: unknown): value is Thread => object(value) && shortString(value.id, 256) && value.id.length > 0 && shortString(value.quote, 8192)
    && [value.prefix, value.suffix].every((field) => field === undefined || shortString(field, 8192))
    && Object.keys(value).every((key) => ['id', 'quote', 'prefix', 'suffix', 'resolved', 'comments'].includes(key))
    && (value.resolved === undefined || typeof value.resolved === 'boolean') && Array.isArray(value.comments) && value.comments.length === 0;
export function validReview(value: unknown): value is ReviewOperation {
    if (!object(value) || Object.keys(value).sort().join(',') !== 'actor,comment,document,id,kind,participant,previous,thread,v,value'
        || value.v !== 1 || ![value.id, value.document, value.actor, value.participant].every(validId) || !shortString(value.thread, 256) || !value.thread
        || !Array.isArray(value.previous) || value.previous.length > 64 || !value.previous.every(validId)
        || typeof value.id !== 'string' || value.previous.includes(value.id)) return false;
    return value.kind === 'undo-delete' ? (value.comment === null || shortString(value.comment, 256)) && validId(value.value) && value.previous.includes(value.value)
        : value.kind === 'ack' ? value.comment === null && typeof value.value === 'string' && /^[a-f0-9]{64}$/.test(value.value) && value.thread === `ack:${value.participant}`
        : value.kind === 'thread' ? value.comment === null && validThread(value.value) && value.value.id === value.thread
        : value.kind === 'comment' ? shortString(value.comment, 256) && validComment(value.value) && value.value.id === value.comment
            : value.kind === 'resolve' ? value.comment === null && typeof value.value === 'boolean'
                : value.kind === 'delete-thread' ? value.comment === null && value.value === null
                    : value.kind === 'delete-comment' && shortString(value.comment, 256) && value.value === null;
}
export async function verifyReview(signed: SignedReview, grant: Grant) {
    const op = signed.operation;
    return validReview(op) && grant.permission !== 'view' && op.actor === grant.member && op.participant === grant.participant
        && (grant.document === null || grant.document === op.document) && await verify(op, signed.signature, grant.publicKey);
}
const target = (op: ReviewOperation) => JSON.stringify([op.thread, op.comment, op.kind === 'resolve' ? 'resolution' : op.comment === null ? 'thread' : 'comment']);

export class ReviewReplica {
    readonly operations = new Map<string, SignedReview>();
    readonly recoveredThreads = new Set<string>();
    readonly document: string;
    readonly seed: Thread[];
    readonly seedParticipant: string | null;
    constructor(document: string, seed: Thread[] = [], seedParticipant: string | null = null) { this.document = document; this.seed = structuredClone(seed); this.seedParticipant = seedParticipant; }
    async apply(signed: SignedReview, grant: Grant) {
        const op = signed.operation;
        if (op.document !== this.document || !(await verifyReview(signed, grant))) return fail('invalid-review-operation', 403);
        const existing = this.operations.get(op.id);
        if (existing && canonical(existing) !== canonical(signed)) return fail('review-operation-reused', 409);
        if (op.kind === 'undo-delete') {
            const deleted = this.operations.get(op.value as string)?.operation;
            if (!deleted || deleted.actor !== op.actor || deleted.participant !== op.participant || deleted.thread !== op.thread || deleted.comment !== op.comment
                || deleted.kind !== (op.comment === null ? 'delete-thread' : 'delete-comment')) return fail('review-undo-not-owned', 403);
        }
        if (op.kind === 'thread') {
            const original = [...this.operations.values()].find(({ operation }) => operation.thread === op.thread && operation.kind === 'thread');
            const historical = this.seed.some((thread) => thread.id === op.thread);
            if (!original && !historical && !op.thread.startsWith(`${op.participant}:`)) return fail('thread-author-mismatch', 403);
            const creator = original?.operation.participant ?? (historical ? this.seedParticipant : op.participant);
            if (grant.permission === 'comment' && creator !== op.participant) return fail('thread-author-mismatch', 403);
        }
        // Moderation deletes remain signed by the moderator. Editing a comment
        // always requires authorship, even for an editor.
        if (op.kind === 'comment' || op.kind === 'delete-comment' && grant.permission === 'comment') {
            const historical = this.seed.some((t) => t.id === op.thread && t.comments.some((c) => commentKey(c) === op.comment));
            // Ownership must be independent of delivery order. A new comment ID binds
            // its participant before any replica receives its first version.
            if (!historical && !op.comment?.startsWith(`${op.participant}:`)) return fail('comment-author-mismatch', 403);
            const original = [...this.operations.values()].find(({ operation: prior }) => prior.thread === op.thread && prior.comment === op.comment && prior.kind === 'comment');
            if (original && original.operation.participant !== op.participant) return fail('comment-author-mismatch', 403);
            if (historical && grant.permission !== 'owner' && grant.participant !== this.seedParticipant) return fail('historical-comment-read-only', 403);
        }
        if (op.kind === 'delete-thread' && grant.permission === 'comment') {
            const creator = [...this.operations.values()].find(({ operation }) => operation.thread === op.thread && operation.kind === 'thread')?.operation.participant
                ?? (this.seed.some((thread) => thread.id === op.thread) ? this.seedParticipant : null);
            if (creator !== op.participant) return fail('thread-owner-required', 403);
            const comments = this.threads().find(thread => thread.id === op.thread)?.comments ?? [];
            if (comments.some(comment => !commentKey(comment).startsWith(`${op.participant}:`)
                && !(this.seedParticipant === op.participant && this.seed.some(thread => thread.id === op.thread && thread.comments.some(seed => commentKey(seed) === commentKey(comment)))))) return fail('thread-has-other-authors', 403);
        }
        this.operations.set(op.id, structuredClone(signed));
    }
    // Causal predecessors, not clocks, determine superseded versions.
    private predecessors(ids: string[]) {
        const known = new Set<string>(), pending = [...ids];
        while (pending.length) {
            const id = pending.pop()!;
            if (known.has(id)) continue;
            known.add(id);
            pending.push(...(this.operations.get(id)?.operation.previous ?? []));
        }
        return known;
    }
    heads(thread: string, comment: string | null, kind: ReviewOperation['kind']) {
        const key = target({ thread, comment, kind } as ReviewOperation);
        const restored = new Set([...this.operations.values()].filter(({ operation }) => operation.kind === 'undo-delete').map(({ operation }) => operation.value as string));
        const candidates = [...this.operations.values()].filter(({ operation }) => operation.kind !== 'undo-delete' && !restored.has(operation.id) && target(operation) === key);
        const superseded = this.predecessors(candidates.flatMap(({ operation }) => operation.previous));
        return candidates.filter(({ operation }) => !superseded.has(operation.id)).sort((a, b) => a.operation.id.localeCompare(b.operation.id));
    }
    private ordered() {
        const date = ({ operation }: SignedReview) => { const at = Date.parse((operation.value as Comment).at ?? ''); return Number.isFinite(at) ? at : 0; };
        const groups = new Map<string, SignedReview[]>(), created = new Map<string, number>();
        for (const signed of this.operations.values()) if (signed.operation.kind === 'comment') {
            const key = target(signed.operation), group = groups.get(key) ?? [];
            group.push(signed); groups.set(key, group);
        }
        for (const [key, group] of groups) {
            const ids = new Set(group.map(signed => signed.operation.id));
            const originals = group.length === 1 ? group : group.filter(({ operation }) => {
                if (operation.previous.some(id => ids.has(id))) return false;
                const ancestors = this.predecessors(operation.previous);
                return !group.some(signed => ancestors.has(signed.operation.id));
            });
            const original = (originals.length ? originals : group).sort((a, b) => date(a) - date(b) || a.operation.id.localeCompare(b.operation.id))[0];
            created.set(key, date(original));
        }
        // Edits retain the first version's creation time. Non-comment actions must
        // not pull a later legacy comment ahead of an earlier independent comment.
        const time = new Map([...this.operations.values()].map(({ operation }) => [operation.id, operation.kind === 'comment' ? created.get(target(operation))! : Infinity] as const));
        const compare = (a: SignedReview, b: SignedReview) => time.get(a.operation.id)! - time.get(b.operation.id)! || a.operation.id.localeCompare(b.operation.id);
        const out: SignedReview[] = [], done = new Set<string>(), visiting = new Set<string>();
        // Dates order independent/legacy comments. Observed predecessors always come
        // first, even with equal timestamps or clocks that disagree. An iterative
        // traversal also bounds deep histories and malformed cyclic references.
        for (const signed of [...this.operations.values()].sort(compare)) {
            const stack = [{ signed, finish: false }];
            while (stack.length) {
                const item = stack.pop()!, id = item.signed.operation.id;
                if (done.has(id)) continue;
                if (item.finish) { visiting.delete(id); done.add(id); out.push(item.signed); continue; }
                if (visiting.has(id)) continue;
                visiting.add(id); stack.push({ signed: item.signed, finish: true });
                const parents = [...new Set(item.signed.operation.previous)].flatMap(id => this.operations.has(id) ? [this.operations.get(id)!] : []).sort(compare);
                for (const parent of parents.reverse()) if (!done.has(parent.operation.id)) stack.push({ signed: parent, finish: false });
            }
        }
        return out;
    }
    threads(): Thread[] {
        this.recoveredThreads.clear();
        const out = new Map(this.seed.map((t) => [t.id, structuredClone(t)]));
        const operations = this.ordered();
        for (const { operation: op } of operations) {
            if (op.kind === 'thread' && !out.has(op.thread)) out.set(op.thread, { ...(op.value as Thread), comments: [] });
        }
        for (const [id, thread] of out) {
            const structural = this.heads(id, null, 'thread');
            const deleted = structural.length > 0 && structural.every(({ operation }) => operation.kind === 'delete-thread');
            const removed = deleted ? this.predecessors(structural.flatMap(({ operation }) => operation.previous)) : new Set<string>();
            const commentKeys = new Set([...thread.comments.map(commentKey), ...operations.filter(({ operation }) => operation.thread === id && operation.kind === 'comment').map(({ operation }) => operation.comment!)]);
            if (deleted) {
                const replies = [...commentKeys].flatMap((key) => this.heads(id, key, 'comment')).filter(({ operation }) => operation.kind === 'comment' && !removed.has(operation.id));
                if (!replies.length) { out.delete(id); continue; }
                // A concurrent reply recovers its thread instead of disappearing behind
                // an owner's deletion. The host can show a recovery note without putting it in Markdown.
                thread.comments = [];
                this.recoveredThreads.add(id);
            } else {
                const metadata = structural.filter(({ operation }) => operation.kind === 'thread').at(-1)?.operation.value as Thread | undefined;
                if (metadata) { const comments = thread.comments; Object.assign(thread, structuredClone(metadata), { comments }); }
            }
            const resolution = this.heads(id, null, 'resolve');
            if (resolution.length) thread.resolved = resolution.every(({ operation }) => operation.value === true);
            const comments = new Map(thread.comments.map(comment => [commentKey(comment), [comment]]));
            for (const key of commentKeys) {
                const current = this.heads(id, key, 'comment');
                if (!current.length) continue; // Cancelling a seed-comment deletion restores the unchanged seed.
                const heads = current.filter(({ operation }) => operation.kind === 'comment' && !removed.has(operation.id));
                comments.set(key, heads.map(({ operation }) => {
                    const value = operation.value as Comment;
                    return { ...value, id: heads.length === 1 ? key : `${key}:${operation.id}` };
                }));
            }
            thread.comments = [...comments.values()].flat();
        }
        return [...out.values()];
    }
    project(original: string) {
        const parsed = splitComments(original), threads = this.threads();
        if (canonical(parsed.threads) === canonical(threads)) return original;
        const joined = joinComments(parsed.body, threads);
        return original.includes('\r\n') ? joined.replace(/\r?\n/g, '\r\n') : joined;
    }
}
