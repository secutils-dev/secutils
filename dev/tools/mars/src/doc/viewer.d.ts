// Host API of the ported md.html viewer (viewer.js).
import type { Comment, Thread } from '../share/merge.ts';
import type { ExportSnapshot } from '../export/snapshot.ts';
import type { ReadingWidth } from '../export/settings.ts';
import type { InputOperation } from '../io/input.ts';

export interface ViewerDoc {
    text: string;
    name: string;
    handle: FileSystemFileHandle | null;
    stamp: number;
    // Browser edits are newer than the linked file.
    dirty: boolean;
    readingWidth?: ReadingWidth;
}

export interface ViewerHost {
    currentId(): string | null;
    persist(doc: ViewerDoc): Promise<void>;
    // New content is replacing a non-empty document; the next persist creates a new entry.
    detach(): void;
    readingWidthChanged?(width: ReadingWidth): Promise<void>;
    temporary?(): boolean;
    connection?(): { permission: 'view' | 'comment' | 'edit' | 'owner'; label: string; participant: string; ownerParticipant?: string; ended?: boolean } | null;
    fileAutoSave?(): boolean;
    undoConnection?(redo: boolean): Promise<void>;
    undoReview?(before: Thread[], after: Thread[], id?: string): Promise<void>;
    connectionSaved?(): void;
    // Extracted source from Open or a drop; converted HTML never retains a writable handle.
    openFile?(file: { name: string; text: string; handle: FileSystemFileHandle | null; stamp: number }, input?: InputOperation): Promise<void>;
    openUrl?(url: string): Promise<void>;
}

export function setHost(host: ViewerHost): void;
export function openDocument(doc?: Partial<ViewerDoc>): void;
export function refreshConnection(): void;
export function enableConnectionSafety(): void;
export function connectionDraft(): { start: number; end: number; text: string } | null;
export function setConnectionDraftConflict(recover: (() => void) | null): void;
export function discardConnectionDraft(): void;
export function hasConnectionDraftConflict(): boolean;
export function canApplyConnection(): boolean;
export function canPublishConnection(): boolean;
export function applyConnectionProjection(doc: Pick<ViewerDoc, 'text' | 'name'> & { readingWidth?: ReadingWidth }): boolean;
export function renameDocument(name: string): void;
export function closeDocument(): void;
export function setViewMode(mode: 'preview' | 'edit'): void;
export function flush(): Promise<void>;
export function endLoading(alert?: boolean): void;
export function currentName(): string;
export function currentText(): string;
export function currentThreads(): Thread[];
export function captureExportSnapshot(): Promise<ExportSnapshot>;
export function exportReadingWidth(): ReadingWidth;
export function defaultReadingWidth(): ReadingWidth;
export function setDocumentReadingWidth(width?: ReadingWidth): void;
export function renderExportBody(body: string, signal?: AbortSignal): Promise<string>;
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
