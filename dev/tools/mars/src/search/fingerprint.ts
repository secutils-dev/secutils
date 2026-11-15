// Index identity follows exact model inputs, not generic document modification timestamps.
// Positions and display snippets can change while the same vectors remain reusable.
import { passageInput, type Model } from './models.ts';
import type { Chunk } from './chunks.ts';

/** SHA-256 of the ordered, exact passage inputs. Names matter only when the selected model uses them. */
export async function inputFingerprint(model: Model, name: string, chunks: Chunk[]) {
    const bytes = new TextEncoder().encode(JSON.stringify(chunks.map((chunk) => passageInput(model, name, chunk))));
    return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), (byte) =>
        byte.toString(16).padStart(2, '0'),
    ).join('');
}
