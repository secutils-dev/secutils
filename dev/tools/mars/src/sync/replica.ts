// Body-only replication adapter. Yjs does not know about transport, keys, or review JSON.
// An update that reaches `apply` must already be authenticated and decrypted.
// This class checks no author and no permission.
// Local edits use the `local` transaction origin. Only those enter the undo stack.
import * as Y from 'yjs';
import { fail } from './protocol.ts';
import { sourceChange, type SourceChange } from '../doc/source-change.ts';
export { sourceChange, type SourceChange } from '../doc/source-change.ts';
/**
 * One Yjs document with the Markdown body in its `body` text. The same document holds the shared maps `meta`, `nodes`,
 * and `identity`. Offsets are UTF-16 code units, as in JavaScript strings.
 */
export class BodyReplica {
    readonly doc = new Y.Doc();
    readonly body = this.doc.getText('body');
    /** Transaction origin of local edits. */
    readonly local = {};
    /** Undo stack of local body edits. With `captureTimeout: 0`, each transaction is one undo step. */
    readonly undo = new Y.UndoManager(this.body, { trackedOrigins: new Set([this.local]), captureTimeout: 0 });
    /** Starts empty, or from a trusted Yjs update. */
    constructor(checkpoint?: Uint8Array) {
        if (checkpoint) Y.applyUpdate(this.doc, checkpoint);
    }
    /**
     * Inserts the initial text. Called only by the creator, once. Joiners MUST bootstrap the creator's checkpoint. Two
     * independent seeds of one text would merge into doubled text.
     * @throws `SyncError` `replica-already-seeded` when the document already has content or history.
     */
    seed(text: string) {
        if (this.body.length || Y.decodeStateVector(Y.encodeStateVector(this.doc)).size)
            return fail('replica-already-seeded');
        this.doc.transact(() => {
            this.doc.getMap('identity').set('seeded', true);
            if (text) this.body.insert(0, text);
        }, 'seed');
    }
    text() {
        return this.body.toString();
    }
    /** Encodes the complete document state as one Yjs update. A later checkpoint contains each earlier one. */
    checkpoint(): Uint8Array<ArrayBuffer> {
        return new Uint8Array(Y.encodeStateAsUpdate(this.doc));
    }
    /** Merges a remote update. The merge is idempotent and independent of order. Undo does not track it. */
    apply(checkpoint: Uint8Array) {
        Y.applyUpdate(this.doc, checkpoint, 'remote');
    }
    /** Merges an update as a local change, so the undo stack tracks it. */
    applyLocal(checkpoint: Uint8Array) {
        Y.applyUpdate(this.doc, checkpoint, this.local);
    }
    /**
     * Replaces the range from `from` to `to` with `insert` in one local transaction.
     * @throws `SyncError` `invalid-source-range` for a range outside the current text.
     */
    edit(change: SourceChange) {
        if (
            !Number.isSafeInteger(change.from) ||
            !Number.isSafeInteger(change.to) ||
            change.from < 0 ||
            change.to < change.from ||
            change.to > this.body.length
        )
            return fail('invalid-source-range');
        this.doc.transact(() => {
            if (change.to > change.from) this.body.delete(change.from, change.to - change.from);
            if (change.insert) this.body.insert(change.from, change.insert);
        }, this.local);
    }
    /** Applies the single-range edit that turns the current text into `text`. */
    replace(text: string) {
        const change = sourceChange(this.text(), text);
        if (change) this.edit(change);
    }
    /**
     * Captures a range as relative positions that follow concurrent edits, with its current text. `from` binds to the
     * character after it and `to` to the character before it, so the range does not grow.
     */
    anchor(from: number, to = from) {
        return {
            from: Y.encodeRelativePosition(Y.createRelativePositionFromTypeIndex(this.body, from, 0)),
            to: Y.encodeRelativePosition(Y.createRelativePositionFromTypeIndex(this.body, to, -1)),
            original: this.text().slice(from, to),
        };
    }
    /** True when the anchored range still resolves and holds the same text. */
    unchanged(anchor: ReturnType<BodyReplica['anchor']>) {
        const range = this.range(anchor);
        return !!range && this.text().slice(range[0], range[1]) === anchor.original;
    }
    /** Resolves an anchor to current offsets. Returns `null` when an end is outside the body or the ends crossed. */
    range(anchor: ReturnType<BodyReplica['anchor']>): [number, number] | null {
        const from = Y.createAbsolutePositionFromRelativePosition(Y.decodeRelativePosition(anchor.from), this.doc);
        const to = Y.createAbsolutePositionFromRelativePosition(Y.decodeRelativePosition(anchor.to), this.doc);
        return from?.type === this.body && to?.type === this.body && to.index >= from.index
            ? [from.index, to.index]
            : null;
    }
    /** Releases the undo manager and the Yjs document. */
    destroy() {
        this.undo.destroy();
        this.doc.destroy();
    }
}
