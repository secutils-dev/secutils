// Export (.md, .zip, a folder on disk) and import (.md files, folders, .zip exports).
import { strToU8, strFromU8, unzipSync, zipSync, type Zippable } from 'fflate';
import * as app from '../app.ts';
import { descendants, exportPaths } from '../tree/model.ts';
import { download, fileSafe } from '../ui/ui.ts';
import { toast } from '../doc/viewer.js';

const MD_RE = /\.(md|markdown|mdx|txt)$/i;
const FRONTMATTER_RE = /^---\r?\n([\s\S]*?)\r?\n---\r?\n/;
const FRONTMATTER_TITLE_RE = /^title:\s*['"]?(.+?)['"]?\s*$/m;
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

const docName = (file: string, text: string) => text.match(FRONTMATTER_RE)?.[1].match(FRONTMATTER_TITLE_RE)?.[1] ?? file.replace(MD_RE, '');

async function expand(file: File, relPath: string): Promise<Incoming[]> {
    const segments = relPath.split('/').filter(Boolean);
    const folders = segments.slice(0, -1);
    if (/\.zip$/i.test(file.name)) {
        const entries = unzipSync(new Uint8Array(await file.arrayBuffer()), { filter: (f) => MD_RE.test(f.name) && !/(^|\/)(__MACOSX|\.)/.test(f.name) });
        return Object.entries(entries).map(([path, bytes]) => {
            const parts = path.split('/').filter(Boolean);
            const text = strFromU8(bytes);
            return { path: [...folders, ...parts.slice(0, -1)], name: docName(parts.at(-1)!, text), text };
        });
    }
    if (!MD_RE.test(file.name)) return [];
    const text = await file.text();
    return [{ path: folders, name: docName(file.name, text), text }];
}

async function importIncoming(items: Incoming[], parentId: string | null, skipped: number) {
    if (!items.length) { toast(skipped ? 'Only .md, .markdown, .mdx, .txt, and .zip files can be imported' : 'Nothing to import'); return; }
    const folderIds = new Map<string, string | null>([['', parentId]]);
    let last: string | null = null;
    for (const item of items) {
        let parent = parentId, key = '';
        for (const seg of item.path) {
            key += `/${seg}`;
            if (!folderIds.has(key)) folderIds.set(key, (await app.findOrCreateFolder(seg, parent)).id);
            parent = folderIds.get(key)!;
        }
        last = (await app.createDoc({ name: app.uniqueName(item.name, parent), text: item.text, parentId: parent, open: false })).id;
    }
    if (items.length === 1 && last) await app.openDoc(last);
    toast(`Imported ${items.length} document${items.length === 1 ? '' : 's'}${skipped ? ` · skipped ${skipped} other file${skipped === 1 ? '' : 's'}` : ''}`);
}

export async function importFiles(files: File[], parentId: string | null = app.state.folderId) {
    const items: Incoming[] = [];
    let skipped = 0;
    for (const file of files) {
        const got = await expand(file, (file as File & { webkitRelativePath?: string }).webkitRelativePath || file.name);
        if (!got.length) skipped++;
        items.push(...got);
    }
    await importIncoming(items, parentId, skipped);
}

// Drops can carry whole folders; walk them with the (Baseline) FileSystemEntry API.
export async function importDataTransfer(dt: DataTransfer, parentId: string | null) {
    const entries = [...dt.items].map((i) => i.webkitGetAsEntry?.()).filter((e): e is FileSystemEntry => !!e);
    if (!entries.length) { await importFiles([...dt.files], parentId); return; }
    const files: File[] = [];
    const walk = async (entry: FileSystemEntry, prefix: string): Promise<void> => {
        if (entry.isFile) {
            const file = await new Promise<File>((resolve, reject) => (entry as FileSystemFileEntry).file(resolve, reject));
            files.push(Object.defineProperty(file, 'webkitRelativePath', { value: `${prefix}${file.name}` }));
            return;
        }
        const reader = (entry as FileSystemDirectoryEntry).createReader();
        for (;;) {
            const batch = await new Promise<FileSystemEntry[]>((resolve, reject) => reader.readEntries(resolve, reject));
            if (!batch.length) break;
            for (const child of batch) await walk(child, `${prefix}${entry.name}/`);
        }
    };
    for (const entry of entries) await walk(entry, '');
    await importFiles(files, parentId);
}
