// Body-only replication adapter. Yjs does not know about transport, keys, or review JSON.
import * as Y from 'yjs';
import { fail } from './protocol.ts';
import { sourceChange, type SourceChange } from '../doc/source-change.ts';
export { sourceChange, type SourceChange } from '../doc/source-change.ts';
export class BodyReplica {
    readonly doc = new Y.Doc();
    readonly body = this.doc.getText('body');
    readonly local = {};
    readonly undo = new Y.UndoManager(this.body, { trackedOrigins: new Set([this.local]), captureTimeout: 0 });
    constructor(checkpoint?: Uint8Array) {
        if (checkpoint) Y.applyUpdate(this.doc, checkpoint);
    }
    // Called only by the creator, once. Joiners MUST bootstrap the creator's checkpoint.
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
    checkpoint(): Uint8Array<ArrayBuffer> {
        return new Uint8Array(Y.encodeStateAsUpdate(this.doc));
    }
    apply(checkpoint: Uint8Array) {
        Y.applyUpdate(this.doc, checkpoint, 'remote');
    }
    applyLocal(checkpoint: Uint8Array) {
        Y.applyUpdate(this.doc, checkpoint, this.local);
    }
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
    replace(text: string) {
        const change = sourceChange(this.text(), text);
        if (change) this.edit(change);
    }
    anchor(from: number, to = from) {
        return {
            from: Y.encodeRelativePosition(Y.createRelativePositionFromTypeIndex(this.body, from, 0)),
            to: Y.encodeRelativePosition(Y.createRelativePositionFromTypeIndex(this.body, to, -1)),
            original: this.text().slice(from, to),
        };
    }
    unchanged(anchor: ReturnType<BodyReplica['anchor']>) {
        const range = this.range(anchor);
        return !!range && this.text().slice(range[0], range[1]) === anchor.original;
    }
    range(anchor: ReturnType<BodyReplica['anchor']>): [number, number] | null {
        const from = Y.createAbsolutePositionFromRelativePosition(Y.decodeRelativePosition(anchor.from), this.doc);
        const to = Y.createAbsolutePositionFromRelativePosition(Y.decodeRelativePosition(anchor.to), this.doc);
        return from?.type === this.body && to?.type === this.body && to.index >= from.index
            ? [from.index, to.index]
            : null;
    }
    destroy() {
        this.undo.destroy();
        this.doc.destroy();
    }
}
