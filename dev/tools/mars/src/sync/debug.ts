// Opt-in, local-only Sync diagnostics. Every exported field has an allowlist.
// Never pass names, Markdown, URLs, credentials, envelopes, or raw errors here.
// A value outside its allowlist is dropped or replaced by a fixed fallback. The trace stays in tab memory.
import { featureFlags, registerFlagFeature } from '../platform/flags.ts';
import { debugReport, registerDebugProvider, clearDebugReport } from '../platform/debug.ts';
/** Fixed names of the enrollment setup steps. A failure report names its step with one of them. */
export const SETUP_STAGES = [
    'initializing',
    'saving-edits',
    'checking-selection',
    'reading-documents',
    'creating-collection',
    'registering-items',
    'preparing-manifest',
    'saving-connections',
    'uploading',
] as const;
/** One of `SETUP_STAGES`. */
export type SetupStage = (typeof SETUP_STAGES)[number];
/** Event names that the trace accepts. `record` ignores any other name. */
const EVENTS = new Set([
    'boot',
    'environment.changed',
    'library.refresh',
    'setup.stage',
    'setup.failed',
    'setup.finished',
    'store.conflict',
    'request.finished',
    'request.failed',
    'transport.status',
    'poll.finished',
]);
/** Fixed reasons of a local storage conflict (`ReplicaConflict`). */
const REASONS = [
    'revision',
    'selection-grown',
    'missing-node',
    'already-connected',
    'orphan-replica',
    'text-changed',
    'metadata-changed',
] as const;
/** One of the conflict reasons. */
export type ConflictReason = (typeof REASONS)[number];
/**
 * Error codes that can enter the trace. Any other code or message becomes `unexpected-error`, so free text never
 * reaches a report.
 */
const CODES = new Set([
    ...REASONS,
    'unexpected-error',
    'temporary-storage',
    'selection-too-large',
    'sync-pending',
    'collection-creation-limit',
    'sync-storage-full',
    'update-too-large',
    'invitation-expired',
    'membership-or-document-limit',
    'renewal-needs-checkpoint',
    'connection-expired',
    'connection-ended',
    'document-deleted',
    'document-expired',
    'access-ended',
    'forbidden',
    'forbidden-stream',
    'forbidden-document',
    'key-epoch-changed',
    'invalid-service-response',
    'service-unavailable',
    'sync-request-failed',
    'response-too-large',
    'acknowledgement-mismatch',
]);
CODES.add('binding-missing');
CODES.add('unfinished-setup-unavailable');
CODES.add('unsupported-link-key-format');
/** Error class names that the trace keeps. Any other name becomes `Error`. */
const ERRORS = new Set([
    'Error',
    'ReplicaConflict',
    'SyncSetupError',
    'SyncError',
    'TypeError',
    'RangeError',
    'SyntaxError',
    'AbortError',
    'NetworkError',
    'NotAllowedError',
    'QuotaExceededError',
    'SecurityError',
    'InvalidStateError',
    'TransactionInactiveError',
    'DataCloneError',
    'VersionError',
    'UnknownError',
]);
/** Service action names that the trace keeps. */
const ACTIONS = new Set([
    'create',
    'document',
    'append',
    'index',
    'get',
    'directory',
    'document-info',
    'touch',
    'touch-record',
    'touch-folder',
    'invite',
    'invite-read',
    'pair-request',
    'pair-status',
    'pair-approve',
    'members',
    'epoch',
    'key-read',
    'key-write',
    'live-key-read',
    'live-key-write',
    'delete-document',
]);
// Allowlists of the other fields: connection states, tree-node field names, integer counters, and booleans.
const STATES = new Set(['waiting', 'working', 'current', 'offline', 'attention', 'ended']);
const FIELDS = new Set(['name', 'parentId', 'order', 'originId', 'updatedAt', 'readingWidth']);
const NUMBERS = new Set([
    'items',
    'documents',
    'folders',
    'connected',
    'storedConnected',
    'orphanRecords',
    'item',
    'status',
    'duration',
    'changed',
    'line',
    'column',
    'downloads',
    'rejected',
]);
const FLAGS = new Set(['temporary', 'hidden', 'online']);
/**
 * Reduces a thrown value to allowlisted fields: the error class name, a fixed code, an HTTP status, and the line and
 * column of the first stack frame. The message counts only when it equals a known code. No file name, URL, or free text
 * passes.
 */
export function safeError(error: unknown) {
    const value = error as {
        name?: unknown;
        code?: unknown;
        reason?: unknown;
        message?: unknown;
        stack?: unknown;
        status?: unknown;
    } | null;
    const code = [value?.reason, value?.code, value?.message].find(
        (code) => typeof code === 'string' && CODES.has(code),
    );
    const frame =
        typeof value?.stack === 'string'
            ? /^(?:\s+at\s|[^\s@]*@(?:blob:|https?:|file:))[^\n]*:(\d+):(\d+)\)?$/m.exec(value.stack)
            : null;
    return {
        error: typeof value?.name === 'string' && ERRORS.has(value.name) ? value.name : 'Error',
        code: typeof code === 'string' ? code : 'unexpected-error',
        ...(typeof value?.status === 'number' && value.status >= 100 && value.status <= 599
            ? { status: value.status }
            : {}),
        ...(frame ? { line: Number(frame[1]), column: Number(frame[2]) } : {}),
    };
}
/**
 * Keeps the fields whose name and value pass an allowlist: integers from 0 to 1e9, booleans, and enumerated strings. It
 * drops each other field.
 */
function safeFields(values: Record<string, unknown>) {
    return Object.fromEntries(
        Object.entries(values).filter(([key, value]) => {
            if (NUMBERS.has(key)) return Number.isSafeInteger(value) && Number(value) >= 0 && Number(value) <= 1e9;
            if (FLAGS.has(key)) return typeof value === 'boolean';
            if (typeof value !== 'string') return false;
            return key === 'stage'
                ? SETUP_STAGES.includes(value as SetupStage)
                : key === 'code'
                  ? CODES.has(value)
                  : key === 'reason'
                    ? REASONS.includes(value as ConflictReason)
                    : key === 'error'
                      ? ERRORS.has(value)
                      : key === 'action'
                        ? ACTIONS.has(value)
                        : key === 'state'
                          ? STATES.has(value)
                          : key === 'field'
                            ? FIELDS.has(value)
                            : false;
        }),
    );
}
/**
 * In-memory trace of the latest 500 sanitized events. Timestamps are milliseconds since the start of the log.
 * @param enabled Read at each `record` call.
 * @param output Receives a copy of each entry while the log is enabled.
 */
export class SyncDebugLog {
    private entries: Record<string, unknown>[] = [];
    private started = Date.now();
    private dropped = 0;
    private enabled: () => boolean;
    private output: (event: Record<string, unknown>) => void;
    constructor(enabled: () => boolean, output: (event: Record<string, unknown>) => void = () => {}) {
        this.enabled = enabled;
        this.output = output;
    }
    /**
     * Adds an event with sanitized fields. It ignores an unknown event name. A disabled log keeps only events with
     * `failure` set.
     */
    record(event: string, values: Record<string, unknown> = {}, failure = false) {
        if (!EVENTS.has(event) || (!failure && !this.enabled())) return;
        const entry = { at: Date.now() - this.started, event, ...safeFields(values) };
        this.entries.push(entry);
        if (this.entries.length > 500) {
            this.entries.shift();
            this.dropped++;
        }
        if (this.enabled()) this.output(structuredClone(entry));
    }
    /** Empties the trace and restarts its clock. */
    clear() {
        this.entries = [];
        this.dropped = 0;
        this.started = Date.now();
    }
    /** Returns a copy of the trace with its start time (ISO 8601) and the number of dropped entries. */
    snapshot() {
        return {
            started: new Date(this.started).toISOString(),
            dropped: this.dropped,
            events: structuredClone(this.entries),
        };
    }
}
// Sync and Live links share the replication engine and its sanitized trace.
registerFlagFeature('live_links');
registerFlagFeature('live');
/** True when the `debug` flag is on for `sync`, `live_links`, or `live`. */
export const debugEnabled = () =>
    featureFlags.enabled('debug', 'sync') ||
    featureFlags.enabled('debug', 'live_links') ||
    featureFlags.enabled('debug', 'live');
/** The Sync trace of this tab. While enabled, it also prints each entry to the console. */
export const syncDebug = new SyncDebugLog(debugEnabled, (entry) => console.debug('[MARS Sync debug]', entry));
registerDebugProvider('sync', {
    snapshot: () => ({ enabled: debugEnabled(), ...syncDebug.snapshot() }),
    clear: () => syncDebug.clear(),
});
/** Installs the legacy `window.marsSyncDebug` helper and records boot, visibility, and connectivity events. */
export function installSyncDebug() {
    // Compatibility only; new tooling uses the generic marsFlags / marsDebug APIs.
    Object.assign(window, {
        marsSyncDebug: { report: debugReport, clear: clearDebugReport, disable: () => featureFlags.disable('debug') },
    });
    syncDebug.record('boot', { online: navigator.onLine, hidden: document.hidden });
    const state = () => syncDebug.record('environment.changed', { online: navigator.onLine, hidden: document.hidden });
    document.addEventListener('visibilitychange', state);
    window.addEventListener('online', state);
    window.addEventListener('offline', state);
}
