// The address-bar copy of the current document in a temporary session, so a reload restores it.
// Owns the fragment format, the size limits, and the guard against overlapping updates.
// It never touches storage. The caller supplies the functions that write the address and report the state.
import { encodeState, type SharedDoc } from './codec.ts';
import { validWidth } from '../export/settings.ts';

/**
 * Longest fragment that the route writes, in characters. Chromium limits a URL to 2 MiB.
 * A longer document reports `too-large`, and the reader must download it.
 */
export const MAX_TEMPORARY_FRAGMENT = 2_000_000;
/**
 * Largest source, in UTF-16 code units, that the route tries to encode. Each save compresses
 * the complete text again, so a larger source skips the encode and reports `too-large`.
 */
export const MAX_AUTOMATIC_SOURCE = 16_000_000;
/**
 * State of the address-bar copy.
 * - `empty`: no document.
 * - `pending`: an encode is in progress.
 * - `ready`: the fragment holds the current document.
 * - `too-large`: the document is over a limit.
 * - `failed`: the encode or the address write threw.
 */
export type TemporaryLinkStatus = 'empty' | 'pending' | 'ready' | 'too-large' | 'failed';

/**
 * Builds the fragment of a temporary document: a full link (`name`, `md`, `o`, optional `w`)
 * plus `t=1`. On reload, `t=1` selects the memory backend. `encode` is replaceable for tests.
 */
export async function temporaryFragment(doc: SharedDoc, encode = encodeState) {
    const params = new URLSearchParams({ name: doc.name, md: await encode(doc.text), o: doc.originId, t: '1' });
    if (validWidth(doc.readingWidth)) params.set('w', String(doc.readingWidth));
    return `#${params}`;
}

/**
 * Keeps the address bar in step with the current temporary document.
 * Only the current document goes into the address bar. Other session documents need export.
 * The encode is asynchronous, so updates can overlap. A generation counter lets only the
 * latest update write. A failure of the encode or of the address write becomes a status, not
 * an exception.
 */
export class TemporaryRoute {
    private generation = 0;
    private write: (hash: string) => void;
    private status: (state: TemporaryLinkStatus) => void;
    private encode: typeof encodeState;
    /**
     * @param write Replaces the fragment of the address bar. It can throw.
     * @param status Receives each state change.
     */
    constructor(write: (hash: string) => void, status: (state: TemporaryLinkStatus) => void, encode = encodeState) {
        this.write = write;
        this.status = status;
        this.encode = encode;
    }

    /** Invalidates an update in progress. The address and the status stay as they are. */
    cancel() {
        this.generation++;
    }
    // Without a document the fragment is `#t=1`, so a reload stays in a temporary session.
    private clear(state: TemporaryLinkStatus) {
        try {
            this.write('#t=1');
            this.status(state);
        } catch {
            this.status('failed');
        }
    }

    /** Writes the fragment for `doc`, or clears it for null. A later `update` or `cancel` wins. */
    async update(doc: SharedDoc | null) {
        const generation = ++this.generation;
        if (!doc) {
            this.clear('empty');
            return;
        }
        if (doc.text.length > MAX_AUTOMATIC_SOURCE) {
            this.clear('too-large');
            return;
        }
        this.status('pending');
        try {
            const hash = await temporaryFragment(doc, this.encode);
            if (generation !== this.generation) return;
            if (hash.length > MAX_TEMPORARY_FRAGMENT) {
                this.clear('too-large');
                return;
            }
            this.write(hash);
            this.status('ready');
        } catch {
            if (generation !== this.generation) return;
            this.clear('failed');
        }
    }
}
