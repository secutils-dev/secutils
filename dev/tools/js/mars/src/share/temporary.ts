import { encodeState, type SharedDoc } from './codec.ts';
import { validWidth } from '../export/settings.ts';

export const MAX_TEMPORARY_FRAGMENT = 2_000_000;
export const MAX_AUTOMATIC_SOURCE = 16_000_000;
export type TemporaryLinkStatus = 'empty' | 'pending' | 'ready' | 'too-large' | 'failed';

export async function temporaryFragment(doc: SharedDoc, encode = encodeState) {
    const params = new URLSearchParams({ name: doc.name, md: await encode(doc.text), o: doc.originId, t: '1' });
    if (validWidth(doc.readingWidth)) params.set('w', String(doc.readingWidth));
    return `#${params}`;
}

// Only the current document goes into the address bar. Other session documents need export.
export class TemporaryRoute {
    private generation = 0;
    private write: (hash: string) => void;
    private status: (state: TemporaryLinkStatus) => void;
    private encode: typeof encodeState;
    constructor(write: (hash: string) => void, status: (state: TemporaryLinkStatus) => void, encode = encodeState) {
        this.write = write; this.status = status; this.encode = encode;
    }

    cancel() { this.generation++; }
    private clear(state: TemporaryLinkStatus) {
        try { this.write('#t=1'); this.status(state); }
        catch { this.status('failed'); }
    }

    async update(doc: SharedDoc | null) {
        const generation = ++this.generation;
        if (!doc) { this.clear('empty'); return; }
        if (doc.text.length > MAX_AUTOMATIC_SOURCE) { this.clear('too-large'); return; }
        this.status('pending');
        try {
            const hash = await temporaryFragment(doc, this.encode);
            if (generation !== this.generation) return;
            if (hash.length > MAX_TEMPORARY_FRAGMENT) { this.clear('too-large'); return; }
            this.write(hash); this.status('ready');
        } catch {
            if (generation !== this.generation) return;
            this.clear('failed');
        }
    }
}
