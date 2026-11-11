// Host API of the ported md.html viewer (viewer.js).
//
// The viewer owns the preview, the source and block editors, review threads, and writes to a
// linked file. The host (main.ts) owns storage, routing, and sharing, and installs its callbacks
// with `setHost`. The viewer is a singleton: it binds to the page DOM when the module loads and
// shows one document at a time.
// - "Text" is the complete document: frontmatter, body, and the trailing `su-comments` block.
//   "Body" is the Markdown between them, with LF line endings.
// - Document text, names, and threads can come from another person. The viewer sanitizes
//   rendered HTML through `preview-safety.ts`.
import type { Comment, Thread } from '../share/merge.ts';
import type { ExportSnapshot } from '../export/snapshot.ts';
import type { ReadingWidth } from '../export/settings.ts';
import type { InputOperation } from '../io/input.ts';

/** The state of one document, as the host stores it and hands it back to `openDocument`. */
export interface ViewerDoc {
    /** The complete document text, with the line endings of its source. */
    text: string;
    name: string;
    /** The linked file for in-place saves (File System Access), or null. */
    handle: FileSystemFileHandle | null;
    /** `lastModified` of the linked file at the last read or write, in milliseconds. 0 when unknown. */
    stamp: number;
    // Browser edits are newer than the linked file.
    dirty: boolean;
    /** A width that the document carries. Without it, the browser preference applies. */
    readingWidth?: ReadingWidth;
}

/**
 * Callbacks from the viewer into the host. The viewer calls each optional method only when the
 * host provides it.
 */
export interface ViewerHost {
    /** Identifier of the open document in the library, or null when no entry exists for it. */
    currentId(): string | null;
    /**
     * Stores the current document. The viewer calls it 500 ms after the last change, or at once for
     * a connected document. It also calls it from `flush` and after a write to the linked file. A
     * rejection shows a toast, and the next change or `flush` calls it again.
     */
    persist(doc: ViewerDoc): Promise<void>;
    // New content is replacing a non-empty document; the next persist creates a new entry.
    detach(): void;
    /** The reader changed the width of a document that carries its own width. */
    readingWidthChanged?(width: ReadingWidth): Promise<void>;
    /** True in a temporary session. The viewer then labels the document and stops automatic file writes. */
    temporary?(): boolean;
    /**
     * The Sync or Live link state of the open document, or null for a local document.
     * `permission` limits the controls: `view` allows no change, `comment` allows review threads,
     * and `edit` and `owner` allow body edits. `label` is the connection status in the save
     * state. `participant` prefixes the identifiers of threads and comments that this member
     * creates, which decides who can edit or delete them. An identifier without a prefix belongs
     * to the owner (`ownerParticipant`).
     */
    connection?(): {
        permission: 'view' | 'comment' | 'edit' | 'owner';
        label: string;
        participant: string;
        ownerParticipant?: string;
        ended?: boolean;
    } | null;
    /** False stops automatic writes to the linked file. An explicit Save still writes it. */
    fileAutoSave?(): boolean;
    /** Undo (or redo) for a connected document. It replaces the local undo history, and runs after `flush`. */
    undoConnection?(redo: boolean): Promise<void>;
    /**
     * Undo of one review action on a connected document. `before` and `after` are the thread
     * lists around that action, and `id` is the document that was open when it happened.
     */
    undoReview?(before: Thread[], after: Thread[], id?: string): Promise<void>;
    /** The state of a connected document is stored, and no `persist` call is in flight. */
    connectionSaved?(): void;
    // Extracted source from Open or a drop; converted HTML never retains a writable handle.
    openFile?(
        file: { name: string; text: string; handle: FileSystemFileHandle | null; stamp: number },
        input?: InputOperation,
    ): Promise<void>;
    /** Loads a document from the URL form. A rejection other than `AbortError` shows its message as a toast. */
    openUrl?(url: string): Promise<void>;
}

/** Replaces the host callbacks. Call it at startup, before the first document opens. */
export function setHost(host: ViewerHost): void;
/**
 * Shows the document that the host selected. Commits an open block editor, cancels a file
 * import, closes Find, and resets the undo history and the review draft. The given state counts
 * as stored: the call triggers no `persist` and no `detach`. `dirty` marks the linked file as
 * older than the text. Without an argument, it shows an empty document named "Untitled".
 *
 * Call `flush` before it. Changes of the previous document that the host has not received are
 * lost.
 */
export function openDocument(doc?: Partial<ViewerDoc>): void;
/**
 * Applies the current `host.connection()` result to the controls: the read-only state of the
 * source editor, the task checkboxes, the block handles, and the save-state label. Call it after
 * the permission or the label changes.
 */
export function refreshConnection(): void;
/**
 * The open block or table-cell draft, or null when no editor is open. `start` and `end` bound
 * the range that the draft replaces, as offsets into the current document text. `text` is the
 * complete document text with the draft applied.
 */
export function connectionDraft(): { start: number; end: number; text: string } | null;
/**
 * Marks the open draft as in conflict with a remote update, or clears the mark with `null`.
 * While the mark is set, the draft editor stays open and never commits, and `currentText` includes
 * the draft. A banner offers a button that calls `recover`.
 */
export function setConnectionDraftConflict(recover: (() => void) | null): void;
/** Closes the open block or cell editor without an edit, and clears a draft conflict. */
export function discardConnectionDraft(): void;
export function hasConnectionDraftConflict(): boolean;
/** True while a block or table-cell editor is open. `flush()` commits that editor. */
export function isEditing(): boolean;
/**
 * True when a remote projection can replace the text without loss. The current state is stored, and
 * no `persist` call for the connection is in flight. No IME composition is active, no block or cell
 * editor is open, and the focus is not in a comment composer.
 */
export function canApplyConnection(): boolean;
/**
 * True when the stored state equals the current state, and neither a `persist` call nor an IME
 * composition is active.
 */
export function canPublishConnection(): boolean;
/**
 * Replaces the text and name with a remote projection. The preview scroll position stays, and
 * the source editor receives the difference as one change. The new state counts as stored, so
 * no `persist` call follows. The preview renders after a short delay.
 *
 * @returns False, without any change, when `canApplyConnection()` is false.
 */
export function applyConnectionProjection(
    doc: Pick<ViewerDoc, 'text' | 'name'> & { readingWidth?: ReadingWidth },
): boolean;
/** Sets the document name without a `persist` call. Use it after the host stored the name. */
export function renameDocument(name: string): void;
/** Opens an empty document named "Untitled". */
export function closeDocument(): void;
/**
 * Switches between Preview and Source (`edit`). A switch away from Preview applies an open cell
 * draft and block editor and closes Find. A switch to Source cancels the current input operation.
 */
export function setViewMode(mode: 'preview' | 'edit'): void;
/**
 * Commits an open block or cell editor, hands unsaved changes to `host.persist`, and waits for
 * that call and for queued file writes.
 *
 * @throws Error when a connected document is still not stored afterwards and no draft conflict
 *   explains it. The caller must not leave the document then.
 */
export function flush(): Promise<void>;
/**
 * Ends the loading placeholder. Without content, the empty state appears.
 *
 * @param alert Also show the notice that a short link expired or does not exist.
 */
export function endLoading(alert?: boolean): void;
export function currentName(): string;
/**
 * The complete current document text. It equals the loaded source byte for byte while the body,
 * the frontmatter, and the threads are unchanged. During a draft conflict it includes the draft.
 */
export function currentText(): string;
/** The live thread list. Treat it as read-only: the viewer owns it. */
export function currentThreads(): Thread[];
/**
 * Captures the open document for export. Applies an open cell draft and block editor first,
 * freezes the identifier, name, text, and reading width, and then persists pending changes.
 * A document that gets its identifier from that persist call carries it in the result.
 */
export function captureExportSnapshot(): Promise<ExportSnapshot>;
/** The reading width that the preview uses at the moment. */
export function exportReadingWidth(): ReadingWidth;
/** The reading width from the browser preference, without a document width. */
export function defaultReadingWidth(): ReadingWidth;
/**
 * Shows the open document with its own width, without a change to the saved browser preference.
 * An absent or invalid width returns the preview to the preference.
 */
export function setDocumentReadingWidth(width?: ReadingWidth): void;
/**
 * Renders a Markdown body to export-safe HTML. The output passed the preview HTML policy. It has no
 * scripts except inert JSON diagram payloads, no event handler attributes, no `javascript:` URLs,
 * and no source-range attributes. Mermaid, KaTeX, and TopoJSON load from their CDN when the body
 * needs them.
 *
 * @param body Markdown without frontmatter and without the comments block.
 * @throws The abort reason of `signal` when it aborts during the render.
 */
export function renderExportBody(body: string, signal?: AbortSignal): Promise<string>;
/**
 * Shows a transient message as plain text: 2.2 seconds, or 6 seconds with an action button.
 * A later call replaces the message.
 */
export function toast(msg: string, action?: { label: string; run: () => void }): void;

// Review changes hooks (MARS only; md.html has none of these).
export interface ThreadHooks {
    // An anchor for a thread whose quote no longer matches the preview.
    fallbackAnchor?(thread: Thread): Range | null;
    // A line shown under the thread's quote.
    note?(thread: Thread): string | null;
    /** True marks the comment with a "New" tag. */
    isNew?(comment: Comment): boolean;
}
/** Registers `fn` for the lifetime of the page. It runs after each preview render, before threads anchor to it. */
export function onRendered(fn: () => void): void;
/**
 * Shows a search hit once the open document has rendered: `query` opens Find with it, and
 * `start` (a body offset) scrolls to the block that contains it. A later call replaces a
 * target that is still pending.
 */
export function reveal(target: { query: string } | { start: number }): void;
/** Replaces the thread hooks. Call `refreshThreads` when their answers change. */
export function setThreadHooks(hooks: ThreadHooks): void;
/** Anchors every thread to the preview again and renders the comments rail. */
export function refreshThreads(): void;
/** The body that the preview DOM was rendered from. `.su-block` `data-start` and `data-end` index into it. */
export function renderedSource(): string;
/** True when the preview shows the current body. False while a render is pending. */
export function inSync(): boolean;
/** The saved comment author name of this browser, or '' when none is saved or storage is unavailable. */
export function currentAuthor(): string;
/**
 * The rendered text of a preview block and a factory for DOM Ranges over `[start, end)` offsets
 * into that text. For a block without text, the Range selects the contents of the block.
 */
export function blockText(el: Element): { text: string; range(start: number, end: number): Range };
/**
 * Quotes threads again from new passages. Each entry holds a thread identifier and a Range in
 * the preview. A longer quote is cut to 2,000 characters, at a space when one exists. Unknown
 * threads and empty ranges are skipped. A change persists like any other review edit.
 */
export function requote(entries: [string, Range][]): void;
