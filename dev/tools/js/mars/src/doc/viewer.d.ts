// Host API of the ported md.html viewer (viewer.js).
import type { Comment, Thread } from '../share/merge.ts';

export interface ViewerDoc {
    text: string;
    name: string;
    handle: FileSystemFileHandle | null;
    stamp: number;
    // Browser edits are newer than the linked file.
    dirty: boolean;
}

export interface ViewerHost {
    currentId(): string | null;
    persist(doc: ViewerDoc): Promise<void>;
    // New content is replacing a non-empty document; the next persist creates a new entry.
    detach(): void;
    // A Markdown file from Open or a drop; the host adds it to the tree or updates its copy.
    openFile?(file: { name: string; text: string; handle: FileSystemFileHandle | null; stamp: number }): Promise<void>;
}

export function setHost(host: ViewerHost): void;
export function openDocument(doc?: Partial<ViewerDoc>): void;
export function renameDocument(name: string): void;
export function closeDocument(): void;
export function setViewMode(mode: 'preview' | 'edit'): void;
export function flush(): Promise<void>;
export function endLoading(alert?: boolean): void;
export function currentName(): string;
export function currentText(): string;
export function currentThreads(): Thread[];
export function toast(msg: string, action?: { label: string; run: () => void }): void;

// Review changes hooks (MARS only; md.html has none of these).
export interface ThreadHooks {
    // An anchor for a thread whose quote no longer matches the preview.
    fallbackAnchor?(thread: Thread): Range | null;
    // A line shown under the thread's quote.
    note?(thread: Thread): string | null;
    isNew?(comment: Comment): boolean;
}
export function onRendered(fn: () => void): void;
export function reveal(target: { query: string } | { start: number }): void;
export function setThreadHooks(hooks: ThreadHooks): void;
export function refreshThreads(): void;
export function renderedSource(): string;
export function inSync(): boolean;
export function currentAuthor(): string;
export function blockText(el: Element): { text: string; range(start: number, end: number): Range };
export function requote(entries: [string, Range][]): void;
