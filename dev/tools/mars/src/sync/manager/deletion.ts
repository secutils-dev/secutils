// Personal Sync deletion: signed fences, atomic local removal/recovery, and a durable upload queue.
// Jobs outlive their last tree row. Visibility pauses requests; reload and exact retries retain intent.
// Unsigned directory flags and expired/revoked access never authorize local deletion.
import * as app from '../../app.ts';
import * as db from '../../store/db.ts';
import * as viewer from '../../doc/viewer.js';
import { descendants, newId } from '../../tree/model.ts';
import { sign } from '../crypto.ts';
import { canonical, LIMITS, SyncError, validId } from '../protocol.ts';
import { deletionAllowed, verifyDeletion, type SignedDeletion } from '../deletion.ts';
import { belongs, driverFor, rootFor, sessions, drivers, preparing, scopeId, request, type Scope } from './state.ts';

/** A persisted deletion job. Credentials never enter an ordinary document export. */
export interface DeletionJob {
    v: 1;
    id: string;
    scope: Scope;
    packets: SignedDeletion[];
}

/** Whether this scope is personal Sync, not a source or recipient connected only through Live links. */
export const personalSync = (scope: Scope) => !scope.live && (scope.personalSync !== false || scope.kind === 'folder');

const verified = new Map<string, string>();
const applying = new Map<string, Promise<void>>();

async function apply(scope: Scope, packets: SignedDeletion[], job?: DeletionJob) {
    const current = app.state.currentId;
    const documents = packets.map((packet) => packet.deletion.document);
    const roots = app.state.nodes.filter(
        (node) => belongs(node, scope) && documents.includes(node.connection!.document),
    );
    const affected = roots
        .flatMap((node) => [node, ...descendants(app.state.nodes, node.id)])
        .filter((node) => belongs(node, scope));
    if (!affected.length && !job) return;
    if (affected.some((node) => node.id === current)) {
        await viewer.flush();
        if (viewer.hasConnectionDraftConflict()) throw new Error('Save the draft before applying the Sync deletion.');
    }
    let recovered = 0;
    await app.serializePersistence(async () => {
        const result = await db.deleteConnected(
            scope.collection,
            scope.member.grant.member,
            documents,
            job ? { id: job.id, value: job } : undefined,
            () => {
                const node = app.current();
                return node?.kind === 'doc' && belongs(node, scope)
                    ? { id: node.id, text: viewer.connectionDraft()?.text ?? viewer.currentText() }
                    : undefined;
            },
        );
        scope.exclusions = result.exclusions;
        recovered = result.recovered;
        for (const id of result.removed) {
            const session = sessions.get(id);
            for (const [operation, pending] of preparing) if (pending === session) preparing.delete(operation);
            sessions.get(id)?.destroy();
            sessions.delete(id);
        }
        await app.load();
    });
    if (!app.state.nodes.some((node) => belongs(node, scope))) {
        driverFor(scope)?.dispose();
        drivers.delete(scopeId(scope));
    }
    if (current && !app.node(current) && !app.state.currentId) app.closeDoc();
    else if (current && app.state.currentId === current && affected.some((node) => node.id === current))
        await app.openDoc(current);
    viewer.refreshConnection();
    if (recovered) viewer.toast('Sync deleted this content. Unsent edits are saved under Recovered Sync copies.');
}

/**
 * Downloads and verifies deletion receipts before uploading local edits. Legacy unsigned fences retain
 * the previous ended-copy behavior. Whole-response validation finishes before the first local write.
 */
export async function synchronizeDeletions(scope: Scope, signal: AbortSignal) {
    if (!personalSync(scope)) return false;
    const transport = driverFor(scope);
    if (!transport) return false;
    const result = await transport.request('deletions', {}, signal);
    if (!Array.isArray(result?.deletions) || result.deletions.length > LIMITS.documents)
        throw new SyncError('invalid-deletions', 503);
    const packets = result.deletions as SignedDeletion[];
    const before = app.state.nodes.filter((node) => belongs(node, scope)).map((node) => node.id);
    for (const packet of packets) {
        const bytes = canonical(packet);
        const key = `${scope.owner.grant.publicKey}:${scopeId(scope)}:${packet?.deletion?.document}`;
        if (verified.get(key) !== bytes) {
            if (!(await verifyDeletion(packet, scope.owner))) throw new SyncError('invalid-deletion-signature', 503);
            verified.set(key, bytes);
        }
    }
    signal.throwIfAborted();
    const previous = applying.get(scopeId(scope)) ?? Promise.resolve();
    const task = previous
        .catch(() => {})
        .then(async () => {
            signal.throwIfAborted();
            await apply(scope, packets);
        });
    applying.set(scopeId(scope), task);
    try {
        await task;
    } finally {
        if (applying.get(scopeId(scope)) === task) applying.delete(scopeId(scope));
    }
    return before.some((id) => !app.node(id)?.connection);
}

/** Saves shared deletion intent with local removal. It works offline and survives removal of the root. */
export async function deleteEverywhere(id: string) {
    const node = app.node(id);
    const scope = node ? rootFor(node) : undefined;
    if (!node || !scope || !personalSync(scope) || !deletionAllowed(scope.member, scope.owner))
        throw new Error('This document is not connected to personal Sync.');
    await viewer.flush();
    if (viewer.hasConnectionDraftConflict()) throw new Error('Save the draft before deleting this document.');
    const nodes = [node, ...descendants(app.state.nodes, id)].filter((node) => belongs(node, scope));
    const packets: SignedDeletion[] = [];
    for (const item of nodes) {
        const deletion: SignedDeletion['deletion'] = {
            v: 1,
            kind: 'mars-sync-delete',
            collection: scope.collection,
            document: item.connection!.document,
            member: scope.member.grant.member,
            operation: newId(),
        };
        packets.push({ deletion, signature: await sign(deletion, scope.identity), author: scope.member });
    }
    // Stop the transport before its binding is removed. A late ACK must not restore a deleted row.
    driverFor(scope)?.stop();
    try {
        await apply(scope, packets, { v: 1, id: newId(), scope: structuredClone(scope), packets });
    } catch (error) {
        driverFor(scope)?.retry();
        throw error;
    }
    resumeDeletions();
    driverFor(scope)?.retry();
}

let running: Promise<void> | undefined;
let controller: AbortController | undefined;
let timer: ReturnType<typeof setTimeout> | undefined;
let failures = 0;
const available = () => !document.hidden && navigator.onLine;

/** Resumes persisted deletion jobs, even when no connected tree rows remain. Errors keep the exact job. */
export function resumeDeletions() {
    if (!available() || running) return;
    clearTimeout(timer);
    controller = new AbortController();
    const signal = controller.signal;
    running = (async () => {
        for (const job of await db.allSides<DeletionJob>('sync-delete')) {
            if (
                job.v !== 1 ||
                !validId(job.id) ||
                !Array.isArray(job.packets) ||
                !job.packets.length ||
                job.packets.length > LIMITS.documents
            )
                throw new SyncError('invalid-deletion-job');
            for (const packet of job.packets) {
                signal.throwIfAborted();
                if (
                    !(await verifyDeletion(packet, job.scope.owner)) ||
                    packet.deletion.member !== job.scope.member.grant.member
                )
                    throw new SyncError('invalid-deletion-job');
                try {
                    await request(
                        'delete-document',
                        {
                            collection: job.scope.collection,
                            member: job.scope.member.grant.member,
                            document: packet.deletion.document,
                            deletion: packet,
                        },
                        job.scope.token,
                        signal,
                    );
                } catch (error) {
                    if (
                        !['connection-expired', 'document-expired', 'connection-ended', 'document-deleted'].includes(
                            (error as Error).message,
                        )
                    )
                        throw error;
                }
            }
            signal.throwIfAborted();
            await db.deleteSide('sync-delete', job.id);
        }
        failures = 0;
    })()
        .catch((error) => {
            if ((error as Error).name === 'AbortError') return;
            failures++;
            viewer.toast('Deletion is saved locally but has not finished syncing.', {
                label: 'Try again',
                run: resumeDeletions,
            });
        })
        .finally(() => {
            running = undefined;
            if (available())
                void db
                    .allSides<DeletionJob>('sync-delete')
                    .then((jobs) => {
                        if (jobs.length)
                            timer = setTimeout(resumeDeletions, Math.min(60000, 2000 * 2 ** Math.min(failures, 5)));
                    })
                    .catch(() => {});
        });
}

const wake = () => {
    if (!available()) {
        controller?.abort();
        clearTimeout(timer);
    } else resumeDeletions();
};
document.addEventListener('visibilitychange', wake);
window.addEventListener('online', wake);
window.addEventListener('offline', wake);
window.addEventListener('pagehide', () => {
    controller?.abort();
    clearTimeout(timer);
});
window.addEventListener('pageshow', wake);
