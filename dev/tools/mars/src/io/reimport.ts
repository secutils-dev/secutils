// Batch imports recognize exact browser-local source paths, never fuzzy content matches.
// Planning writes nothing. Decisions precede folder creation; stale local writes abort atomically.
// A source identity stays local to this browser and never becomes a share or Sync origin.
// An import that matches only another destination asks before it updates that destination.
import * as app from '../app.ts';
import * as db from '../store/db.ts';
import * as sync from '../sync/index.ts';
import * as viewer from '../doc/viewer.js';
import { incomingChoice, sourceBody } from '../share/reconcile.ts';
import { mergeDocuments, openThreads } from '../share/merge.ts';
import { incomingFolder } from './destination.ts';
import { $, esc, modal } from '../ui/ui.ts';
import type { InputOperation } from './input.ts';
import { ancestors, textBytes, type TreeNode } from '../tree/model.ts';

/** Extracted source with display folders and an exact source path, including the filename extension. */
export interface ImportItem {
    path: string[];
    name: string;
    text: string;
    sourcePath: string[];
}
/** The reader's body choice. Copies have fresh identity and leave the existing source baseline unchanged. */
export type ImportChoice = 'mine' | 'theirs' | 'copy';
/** Prepared document and its last received source. A final write checks all three against storage. */
export interface ImportMatch {
    node: TreeNode;
    text: string;
    opened?: string;
}
/** One import action. Missing or ambiguous identity adds a document instead of overwriting one. */
export interface ImportPlan {
    item: ImportItem;
    source?: NonNullable<TreeNode['importSource']>;
    match?: ImportMatch;
    choice: ImportChoice;
    question?: 'legacy' | 'conflict';
    ambiguous?: boolean;
}

/** Collision-free, case-sensitive source key. Includes destination and each complete path segment. */
export const importKey = (source: NonNullable<TreeNode['importSource']>) =>
    JSON.stringify([source.destinationId, source.path]);

/** Exact legacy location, only when every folder segment resolves uniquely. No folder is created. */
export function legacyImportParent(nodes: TreeNode[], destinationId: string | null, path: string[]) {
    let parent = destinationId;
    for (const name of path) {
        const matches = nodes.filter((n) => n.kind === 'folder' && n.parentId === parent && n.name === name);
        if (matches.length !== 1) return undefined;
        parent = matches[0].id;
    }
    return parent;
}

/**
 * The one other destination that recorded some of these source paths, and how many, when `destinationId`
 * recorded none. Undefined when the chosen destination matches, none or several match, or the other no longer accepts imports.
 */
export function earlierDestination(nodes: TreeNode[], items: ImportItem[], destinationId: string | null) {
    const paths = new Set(items.map((item) => JSON.stringify(item.sourcePath)));
    const found = new Map<string | null, number>();
    for (const n of nodes)
        if (n.kind === 'doc' && n.importSource && paths.has(JSON.stringify(n.importSource.path)))
            found.set(n.importSource.destinationId, (found.get(n.importSource.destinationId) ?? 0) + 1);
    if (found.size !== 1 || found.has(destinationId)) return undefined;
    const [[id, count]] = found;
    return id === null || incomingFolder(nodes, id) ? { id, count } : undefined;
}

/** Asks whether a re-import from another destination updates its earlier copies. Undefined cancels. */
export async function chooseDestination(items: ImportItem[], destinationId: string | null, input: InputOperation) {
    const earlier = earlierDestination(app.state.nodes, items, destinationId);
    if (!earlier) return destinationId;
    const label = (id: string | null) =>
        id === null
            ? 'the top level'
            : `"${[...ancestors(app.state.nodes, id), app.node(id)!].map((n) => n.name).join(' / ')}"`;
    const answer = await modal({
        title: 'Update the earlier import?',
        signal: input.signal,
        body: `${earlier.count} of these files were imported into ${label(earlier.id)} before. Update them there, or add new copies to ${label(destinationId)}.`,
        buttons: [
            { label: 'Cancel', value: '' },
            { label: 'Add copies here', value: 'here' },
            { label: 'Update earlier import', value: 'earlier', primary: true },
        ],
    });
    input.check();
    if (!answer.value) return undefined;
    return answer.value === 'earlier' ? earlier.id : destinationId;
}

/**
 * Current folders of recorded documents, keyed like import folder paths (`/A/B`), so new files follow renamed folders.
 * A key maps only when every recorded document at that depth agrees and the folder accepts imports.
 */
export function recordedFolders(nodes: TreeNode[], destinationId: string | null) {
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const found = new Map<string, Set<string>>();
    for (const n of nodes) {
        if (n.kind !== 'doc' || !n.importSource || n.importSource.destinationId !== destinationId) continue;
        const folders = n.importSource.path.slice(0, -1),
            chain: string[] = [];
        let parent = n.parentId;
        for (; parent && chain.length < folders.length; parent = byId.get(parent)?.parentId ?? null)
            chain.unshift(parent);
        // A document moved to another depth or out of the destination does not describe its source folders.
        if (chain.length !== folders.length || parent !== destinationId) continue;
        chain.forEach((id, i) => {
            const key = folders
                .slice(0, i + 1)
                .map((s) => `/${s}`)
                .join('');
            found.set(key, (found.get(key) ?? new Set()).add(id));
        });
    }
    const out = new Map<string, string>();
    for (const [key, ids] of found) {
        const [id] = ids;
        if (ids.size === 1 && incomingFolder(nodes, id)) out.set(key, id);
    }
    return out;
}

/** Plans an import against current storage. Repeated indistinguishable paths never authorize a merge. */
export async function planImport(items: ImportItem[], destinationId: string | null, input: InputOperation) {
    const counts = new Map<string, number>();
    const sourceFor = (item: ImportItem) => ({ destinationId, path: item.sourcePath });
    for (const item of items) {
        const key = importKey(sourceFor(item));
        counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    const byKey = new Map<string, TreeNode[]>();
    for (const n of app.state.nodes)
        if (n.kind === 'doc' && n.importSource) {
            const key = importKey(n.importSource);
            byKey.set(key, [...(byKey.get(key) ?? []), n]);
        }
    const plans: ImportPlan[] = [];
    for (const item of items) {
        input.check();
        const source = sourceFor(item),
            key = importKey(source);
        const recorded = byKey.get(key) ?? [];
        if (counts.get(key)! > 1 || recorded.length > 1) {
            plans.push({ item, choice: 'copy', ambiguous: true });
            continue;
        }
        const parent = recorded.length ? undefined : legacyImportParent(app.state.nodes, destinationId, item.path);
        const legacy =
            parent === undefined
                ? []
                : app.state.nodes.filter(
                      (n) =>
                          n.kind === 'doc' &&
                          n.parentId === parent &&
                          n.name === item.name &&
                          n.opened &&
                          !n.importSource &&
                          !n.handle,
                  );
        const candidate = recorded[0] ?? (legacy.length === 1 ? legacy[0] : undefined);
        const snapshot = candidate ? await db.getDocument(candidate.id) : undefined;
        const opened = snapshot ? await db.getSide('opened', snapshot.node.id) : undefined;
        input.check();
        if (!snapshot) {
            plans.push({ item, source, choice: 'copy' });
            continue;
        }
        const automatic = incomingChoice(snapshot.text, item.text, opened);
        const question = !recorded.length ? 'legacy' : automatic === 'conflict' ? 'conflict' : undefined;
        plans.push({
            item,
            source,
            match: { ...snapshot, opened },
            choice: question ? 'copy' : (automatic as ImportChoice),
            question,
        });
    }
    return plans;
}

/** Collects all conflicts and legacy identity confirmations in one native dialog. Cancellation writes nothing. */
export async function decideImport(plans: ImportPlan[], input: InputOperation) {
    const questions = plans.filter((p) => p.question);
    if (!questions.length) return true;
    const options =
        '<option value="copy">Save as a copy</option><option value="mine">Keep my text</option><option value="theirs">Review changes</option>';
    const pending = modal({
        title: 'Review imported files',
        signal: input.signal,
        html: `<p>Choose what to do with ${questions.length} existing document${questions.length === 1 ? '' : 's'}. Other files will be added or updated automatically.</p>
            <p id="importChoiceHelp">Copies leave existing documents unchanged. Keep my text merges comments only. Review changes takes the incoming text, merges comments, and keeps the previous text until Mark reviewed. No missing files are deleted.</p>
            <label class="field-label" for="importAllChoices">For all listed files</label><select class="select" id="importAllChoices" aria-describedby="importChoiceHelp">${options}</select>
            <div class="import-decisions">${questions
                .map(
                    (
                        p,
                        i,
                    ) => `<div class="import-decision"><label class="field-label" for="importChoice${i}">${esc(p.item.sourcePath.join('/'))}</label>
                <p id="importReason${i}">${p.question === 'legacy' ? 'An older import has this name and folder but no saved file identity. Confirm only if this is the same source file.' : 'Both the source and your MARS text changed.'}</p>
                <select class="select" id="importChoice${i}" aria-describedby="importReason${i} importChoiceHelp">${options}</select></div>`,
                )
                .join('')}</div>`,
        buttons: [
            { label: 'Cancel', value: '' },
            { label: 'Import files', value: 'import', primary: true },
        ],
    });
    const selects = questions.map((_, i) => $<HTMLSelectElement>(`importChoice${i}`));
    $<HTMLSelectElement>('importAllChoices').onchange = (event) => {
        for (const select of selects) select.value = (event.target as HTMLSelectElement).value;
    };
    const answer = await pending;
    input.check();
    if (answer.value !== 'import') return false;
    questions.forEach((plan, i) => {
        plan.choice = selects[i].value as ImportChoice;
    });
    return true;
}

/** Updates a matched copy without selecting it. A changed stored copy is retained and reported for retry. */
export async function updateImport(plan: ImportPlan, input: InputOperation): Promise<'updated' | 'unchanged' | 'kept'> {
    const { match, item, source } = plan;
    if (!match) throw new Error('import-match-required');
    await viewer.flush();
    input.check();
    const latest = await db.getDocument(match.node.id),
        opened = await db.getSide('opened', match.node.id);
    input.check();
    if (
        !latest ||
        latest.text !== match.text ||
        opened !== match.opened ||
        JSON.stringify(latest.node) !== JSON.stringify(match.node)
    )
        throw new db.ReplicaConflict('text-changed');
    if (latest.node.parentId !== null && !incomingFolder(app.state.nodes, latest.node.parentId))
        throw new Error('The document folder is no longer editable. Its current copy is retained.');
    const merged = mergeDocuments(match.text, item.text);
    const text = plan.choice === 'theirs' ? merged.theirs : merged.mine;
    const bodyChanged = sourceBody(match.text) !== sourceBody(text);
    const result = text !== match.text ? 'updated' : sourceBody(item.text) !== sourceBody(text) ? 'kept' : 'unchanged';
    const extras: Record<string, unknown> = {};
    if (opened !== item.text) extras[`opened:${match.node.id}`] = item.text;
    let node: TreeNode = { ...latest.node, importSource: source, opened: true };
    if (bodyChanged && !node.pending) {
        node.pending = true;
        extras[`base:${node.id}`] = match.text;
    }
    if (node.connection && text !== match.text) {
        // A refused connected edit must not bypass its grant or signed-author checks.
        await sync.snapshot(node, text, { expected: match, text: item.text, source, check: () => input.check() });
        input.check();
        const committed = await db.getDocument(node.id);
        if (!committed) throw new db.ReplicaConflict('missing-node');
        // The CRDT may retain concurrent edits; never replace its projection with the prepared text.
        node = committed.node;
    } else {
        if (text !== match.text)
            Object.assign(node, {
                size: textBytes(text),
                threads: openThreads(text),
                updatedAt: Date.now(),
                ...(node.handle ? { dirty: true } : {}),
            });
        if (JSON.stringify(node) === JSON.stringify(latest.node) && text === match.text && !Object.keys(extras).length)
            return result;
        await db.putNodes([node], text === match.text ? {} : { [node.id]: text }, { expected: [match], extras });
    }
    input.check();
    const current = app.node(node.id);
    if (current) Object.assign(current, node);
    if (node.id === app.state.currentId && !node.connection && text !== match.text)
        viewer.openDocument({
            text,
            name: node.name,
            handle: node.handle ?? null,
            stamp: node.stamp ?? 0,
            dirty: !!node.dirty,
            readingWidth: node.readingWidth,
        });
    app.notify();
    return result;
}
