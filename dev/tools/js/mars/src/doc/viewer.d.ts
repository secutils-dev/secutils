// Host API of the ported md.html viewer (viewer.js).
import type { Thread } from '../share/merge.ts';

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
