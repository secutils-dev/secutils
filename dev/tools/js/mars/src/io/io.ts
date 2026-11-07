// Export (.md, .zip, a folder on disk) and import (.md files, folders, .zip exports).
import { strToU8, strFromU8, unzipSync, zipSync, type Zippable } from 'fflate';
import * as app from '../app.ts';
import * as search from '../search/embed.ts';
import { descendants, exportPaths } from '../tree/model.ts';
import { download, fileSafe } from '../ui/ui.ts';
import { toast } from '../doc/viewer.js';
import { readSource } from './source.ts';
import { beginInput, type InputOperation } from './input.ts';
import { receiveFile } from '../share/share.ts';

const SOURCE_FILES_RE = /\.(md|markdown|mdx|txt|html?)$/i;
const today = () => new Date().toISOString().slice(0, 10);

export async function downloadDoc(id: string) {
    const n = app.node(id);
    if (n?.kind !== 'doc') return;
    download(`${fileSafe(n.name)}.md`, await app.docText(id), 'text/markdown;charset=utf-8');
    toast(`Downloaded ${fileSafe(n.name)}.md`);
}

// Every document under `rootId` (null: everything) keyed by its relative export path.
async function collect(rootId: string | null) {
    const paths = exportPaths(app.state.nodes, rootId);
    const scope = rootId ? descendants(app.state.nodes, rootId) : app.state.nodes;
    const files: { path: string; text: string | null }[] = [];
    for (const n of scope) {
        const path = paths.get(n.id);
        if (!path) continue;
        if (n.kind === 'doc') files.push({ path, text: await app.docText(n.id) });
        else if (!scope.some((c) => c.parentId === n.id)) files.push({ path: `${path}/`, text: null });
    }
    return files;
}

const rootName = (rootId: string | null) => (rootId ? fileSafe(app.node(rootId)?.name ?? 'Folder') : `MARS documents ${today()}`);

export async function exportZip(rootId: string | null) {
    const files = await collect(rootId);
    if (!files.some((f) => f.text !== null)) { toast('Nothing to export yet'); return; }
    const zip: Zippable = {};
    for (const f of files) zip[f.path] = f.text === null ? new Uint8Array() : strToU8(f.text);
    const name = `${rootName(rootId)}.zip`;
    download(name, zipSync(zip, { level: 6 }) as BlobPart, 'application/zip');
    toast(`Downloaded ${name}`);
}

// Chromium only: writes the folder as real files into a directory the user picks.
export async function exportToDirectory(rootId: string | null) {
    let dir: FileSystemDirectoryHandle;
    try { dir = await (window as unknown as { showDirectoryPicker(o: object): Promise<FileSystemDirectoryHandle> }).showDirectoryPicker({ mode: 'readwrite' }); }
    catch (err) { if ((err as Error)?.name !== 'AbortError') toast(`Export failed: ${(err as Error)?.message ?? err}`); return; }
    const base = await dir.getDirectoryHandle(rootName(rootId), { create: true });
    let count = 0;
    for (const f of await collect(rootId)) {
        const parts = f.path.split('/').filter(Boolean);
        let d = base;
        for (const p of f.text === null ? parts : parts.slice(0, -1)) d = await d.getDirectoryHandle(p, { create: true });
        if (f.text === null) continue;
        const w = await (await d.getFileHandle(parts.at(-1)!, { create: true })).createWritable();
        await w.write(f.text);
        await w.close();
        count++;
    }
    toast(`Exported ${count} document${count === 1 ? '' : 's'} to ${base.name}`);
}

// ---- Import ----
type Incoming = { path: string[]; name: string; text: string };

export const docName = (file: string, text: string) => readSource(text, { fileName: file }).name;

async function expand(file: File, relPath: string): Promise<Incoming[]> {
    const segments = relPath.split('/').filter(Boolean);
    const folders = segments.slice(0, -1);
    if (/\.zip$/i.test(file.name)) {
        const entries = unzipSync(new Uint8Array(await file.arrayBuffer()), { filter: (f) => SOURCE_FILES_RE.test(f.name) && !/(^|\/)(__MACOSX|\.)/.test(f.name) });
        return Object.entries(entries).map(([path, bytes]) => {
            const parts = path.split('/').filter(Boolean);
            // ZIP entry names are the saved document names from Export all, not inferred titles.
            const source = readSource(strFromU8(bytes), { name: parts.at(-1)!, fileName: parts.at(-1)! });
            return { path: [...folders, ...parts.slice(0, -1)], name: source.name, text: source.text };
        });
    }
    if (!SOURCE_FILES_RE.test(file.name)) return [];
    const source = readSource(await file.text(), { fileName: file.name });
    return [{ path: folders, name: source.name, text: source.text }];
}

async function importIncoming(items: Incoming[], parentId: string | null, skipped: number, input: InputOperation) {
    if (!items.length) { toast(skipped ? 'Only Markdown, HTML, and ZIP files can be imported' : 'Nothing to import'); return; }
    const folderIds = new Map<string, string | null>([['', parentId]]);
    // A folder deleted while the import runs stays deleted: its remaining files are left out.
    const alive = (id: string | null): boolean => { const n = app.node(id); return id === null || (!!n && alive(n.parentId)); };
    const docs: string[] = [];
    try {
        for (const item of items) {
            input.check();
            let parent = parentId, key = '';
            for (const seg of item.path) {
                if (!alive(parent)) break;
                key += `/${seg}`;
                if (!folderIds.has(key)) folderIds.set(key, (await app.findOrCreateFolder(seg, parent)).id);
                input.check();
                parent = folderIds.get(key)!;
            }
            if (!alive(parent)) continue;
            docs.push((await app.createDoc({ name: app.uniqueName(item.name, parent), text: item.text, parentId: parent, open: false, opened: true, input })).id);
        }
    } finally {
        // A delete that lands while a folder or document is being saved leaves it under a deleted folder.
        for (const id of [...folderIds.values(), ...docs]) if (id && app.node(id) && !alive(id)) await app.remove(id);
    }
    input.check();
    const imported = docs.filter((id) => app.node(id)).length, removed = items.length - imported;
    const last = docs.at(-1);
    if (items.length === 1 && last && app.node(last)) await app.openDoc(last, input);
    const notes = [
        removed && `left out ${removed} deleted during the import`,
        skipped && `skipped ${skipped} other file${skipped === 1 ? '' : 's'}`,
    ].filter(Boolean);
    const many = imported >= 20 && search.enabledModel() && !search.isPaused();
    toast(
        `Imported ${imported} document${imported === 1 ? '' : 's'}${notes.map((n) => ` · ${n}`).join('')}`,
        many ? { label: 'Pause indexing', run: () => { search.pause(); toast('Indexing paused. Resume it from the sparkles button in the search box.'); } } : undefined,
    );
}

export async function importFiles(files: File[], parentId: string | null = app.state.folderId, input: InputOperation = beginInput()) {
    input.check();
    try {
        if (files.length === 1 && SOURCE_FILES_RE.test(files[0].name) && !(files[0] as File & { webkitRelativePath?: string }).webkitRelativePath?.includes('/')) {
            const file = files[0], source = readSource(await file.text(), { fileName: file.name });
            input.check();
            const previous = new Set(app.state.nodes.map((node) => node.id));
            await receiveFile({ ...source, handle: null, stamp: file.lastModified, parentId }, input);
            input.check();
            if (app.state.nodes.some((node) => node.kind === 'doc' && !previous.has(node.id))) toast('Imported 1 document');
            return;
        }
        const items: Incoming[] = [];
        let skipped = 0;
        for (const file of files) {
            const got = await expand(file, (file as File & { webkitRelativePath?: string }).webkitRelativePath || file.name);
            input.check();
            if (!got.length) skipped++;
            items.push(...got);
        }
        await importIncoming(items, parentId, skipped, input);
    } catch (error) { if (input.current() && (error as Error).name !== 'AbortError') toast(`Failed to import: ${(error as Error).message}`); }
}

// Drops can carry whole folders; walk them with the (Baseline) FileSystemEntry API.
export async function importDataTransfer(dt: DataTransfer, parentId: string | null) {
    const input = beginInput();
    try {
        const entries = [...dt.items].map((i) => i.webkitGetAsEntry?.()).filter((e): e is FileSystemEntry => !!e);
        if (!entries.length) { await importFiles([...dt.files], parentId, input); return; }
        const files: File[] = [];
        const walk = async (entry: FileSystemEntry, prefix: string): Promise<void> => {
            input.check();
            if (entry.isFile) {
                const file = await new Promise<File>((resolve, reject) => (entry as FileSystemFileEntry).file(resolve, reject));
                input.check();
                files.push(Object.defineProperty(file, 'webkitRelativePath', { value: `${prefix}${file.name}` }));
                return;
            }
            const reader = (entry as FileSystemDirectoryEntry).createReader();
            for (;;) {
                const batch = await new Promise<FileSystemEntry[]>((resolve, reject) => reader.readEntries(resolve, reject));
                input.check();
                if (!batch.length) break;
                for (const child of batch) await walk(child, `${prefix}${entry.name}/`);
            }
        };
        for (const entry of entries) await walk(entry, '');
        await importFiles(files, parentId, input);
    } catch (error) { if (input.current() && (error as Error).name !== 'AbortError') toast(`Failed to import: ${(error as Error).message}`); }
}
