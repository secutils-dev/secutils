// Browser-local scheduling for folder connections. A deadline never authorizes a remote write.
// Settings and edit timestamps are shared by tabs, but do not enter a connection's portable state.

const PREFIX = 'mars.syncSchedule:';
const MINUTE_MS = 60_000;
const PERSONAL = [5_000, 15_000, MINUTE_MS, 5 * MINUTE_MS, 15 * MINUTE_MS, 'manual'] as const;
const GITHUB = [5 * MINUTE_MS, 15 * MINUTE_MS, 30 * MINUTE_MS, 'manual'] as const;

/** A delay in milliseconds, or no automatic work in that direction. */
export type Cadence = number | 'manual';
/** Independent automatic receive and outgoing quiet periods. */
export interface SyncSchedule {
    receive: Cadence;
    send?: Cadence;
}
/** Allowed delays, with the default first. No send list means the connection cannot send. */
export interface ScheduleOptions {
    receive: readonly Cadence[];
    send?: readonly Cadence[];
}

/** Local key for a personal folder member; paired devices have independent settings. */
export const personalScheduleKey = (collection: string, member: string) => `${PREFIX}personal:${collection}:${member}`;
/** Local key for a GitHub working folder. */
export const githubScheduleKey = (id: string) => `${PREFIX}github:${id}`;
/** Options for a personal Sync folder. */
export const personalSchedule: ScheduleOptions = { receive: PERSONAL, send: PERSONAL };
/** Options for a GitHub folder, with the anonymous receive budget and optional write access. */
export const githubSchedule = (authenticated: boolean, writable: boolean): ScheduleOptions => ({
    receive: [authenticated ? MINUTE_MS : 3 * MINUTE_MS, ...GITHUB],
    send: authenticated && writable ? [MINUTE_MS, ...GITHUB] : undefined,
});

function read(key: string): string | null {
    try {
        return globalThis.localStorage?.getItem(key) ?? null;
    } catch {
        return null;
    }
}

const changes = new EventTarget();
function write(key: string, value: string) {
    localStorage.setItem(key, value);
    changes.dispatchEvent(new Event(key));
}

function savedSchedule(key: string): Partial<SyncSchedule> {
    try {
        const parsed = JSON.parse(read(key) ?? '{}');
        return parsed && typeof parsed === 'object' ? parsed : {};
    } catch {
        return {};
    }
}

/** Reads the effective schedule, falling back to defaults for missing or unsupported values. */
export function readSchedule(key: string, options: ScheduleOptions): SyncSchedule {
    const saved = savedSchedule(key);
    return {
        receive: options.receive.includes(saved.receive!) ? saved.receive! : options.receive[0],
        send: options.send?.includes(saved.send!) ? saved.send : options.send?.[0],
    };
}

/** Saves validated choices and notifies this tab. The browser notifies the other tabs. */
export function saveSchedule(key: string, options: ScheduleOptions, value: SyncSchedule) {
    if (!options.receive.includes(value.receive) || (options.send && !options.send.includes(value.send!)))
        throw new Error('Choose an available interval.');
    try {
        write(key, JSON.stringify({ ...savedSchedule(key), ...value }));
    } catch {
        throw new Error('Could not save Sync settings in this browser.');
    }
}

/** Watches settings and outgoing edit markers, in this tab and other tabs. */
export function watchSchedule(key: string, update: () => void, signal: AbortSignal) {
    for (const name of [key, `${key}:edited`, `${key}:sent`]) changes.addEventListener(name, update, { signal });
    window.addEventListener(
        'storage',
        (event) => {
            if (event.key === null || [key, `${key}:edited`, `${key}:sent`].includes(event.key)) update();
        },
        { signal },
    );
}

// Include a nonce so simultaneous saves from different tabs cannot acknowledge each other by timestamp alone.
const volatileEdits = new Map<string, string>();
const volatileSends = new Map<string, string>();
/** Records a locally saved change and restarts the scope's quiet period. */
export function noteSyncEdit(key: string) {
    const value = `${Date.now()}:${crypto.randomUUID()}`;
    try {
        volatileEdits.delete(key);
        write(`${key}:edited`, value);
    } catch {
        volatileEdits.set(key, value);
        changes.dispatchEvent(new Event(`${key}:edited`));
    }
}

/** Returns the last edit marker and its timestamp, or the epoch for previously saved work. */
export function syncEdit(key: string) {
    const marker = volatileEdits.get(key) ?? read(`${key}:edited`) ?? '';
    return { marker, at: Number(marker.split(':')[0]) || 0 };
}

/** True until a GitHub send has examined the latest edit (including working copies from older versions). */
export const githubSendPending = (key: string) =>
    (volatileSends.get(key) ?? read(`${key}:sent`)) !== syncEdit(key).marker;

function saveGitHubSend(key: string, marker: string) {
    try {
        volatileSends.delete(key);
        write(`${key}:sent`, marker);
    } catch {
        // Avoid retrying every idle tick if the browser blocks preference storage.
        volatileSends.set(key, marker);
        changes.dispatchEvent(new Event(`${key}:sent`));
    }
}

/** Acknowledges only the edit marker observed before a successful GitHub send. */
export function acknowledgeGitHubSend(key: string, marker: string) {
    if (syncEdit(key).marker === marker) saveGitHubSend(key, marker);
}

/** Makes previously blocked GitHub work due again without restarting its edit quiet period. */
export function resumeGitHubSend(key: string) {
    saveGitHubSend(key, 'pending');
}
