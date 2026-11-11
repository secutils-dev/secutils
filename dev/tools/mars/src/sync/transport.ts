// Short, scoped polls. Visibility aborts reads, not an upload already in progress.
// Credentials travel only in Authorization headers. Correctness does not use pagehide.
// The transport owns scheduling, backoff, and response size limits. It holds no document key and decrypts nothing.
// Each response is untrusted. The transport checks shapes and the author grant.
// The replica verifies signatures and content.
import { verify } from './crypto.ts';
import {
    LIMITS,
    RejectedPacket,
    SyncError,
    validCheckpoint,
    validGrant,
    validId,
    type SignedCheckpoint,
} from './protocol.ts';
import { syncDebug, safeError } from './debug.ts';
import type { Grant, SignedGrant, Stream, SyncBinding, SyncDirectory } from './protocol.ts';

/**
 * Connection state for the UI.
 * - `waiting`: changes are saved locally and an upload or a retry is due.
 * - `working`: an upload is in progress.
 * - `current`: the outbox is empty after an upload pass.
 * - `offline`: the browser reports no network.
 * - `attention`: the service rejected a request and a user action is necessary. The transport stopped.
 * - `ended`: access ended (HTTP 401, 403, or 410). The transport stopped.
 */
export type ConnectionStatus = 'waiting' | 'working' | 'current' | 'offline' | 'attention' | 'ended';
/** Page signals that gate network use. The default reads `document`, `navigator`, and `window`. */
interface Environment {
    visible(): boolean;
    online(): boolean;
    visibility: EventTarget;
    connectivity: EventTarget;
}
/**
 * Configuration of one transport.
 * - `origin`: service origin. HTTPS, or HTTP on `localhost` and `127.0.0.1`. No path, query, fragment, or credentials.
 * - `token`: bearer token of this member.
 * - `ownerPublicKey`: trusted owner key. It verifies author grants when `authorValid` is absent.
 * - `replica`: outbox and default receive path. Its `acknowledge` gets service values and must verify the digest.
 * - `status`: receives each state change. `projection`: receives the text after an applied checkpoint.
 * - `streams`: streams to download. The default is `['body']`.
 * - `collectionScope`: poll the whole collection, not only the bound document.
 * - `directory`: request the document directory with the first index page.
 * - `known(key)`: true when durable progress covers the checkpoint. The transport then skips the download.
 * - `wanted(key)`: false skips a checkpoint for this poll. A later poll offers it again.
 * - `authorValid`: replaces the default owner-signature check of an author grant.
 * - `receive`: replaces `replica.receive`. `null` means not applied: a later poll offers the checkpoint again.
 * - `cycle`: runs after each complete index scan.
 * - `exclude`: document IDs that the index request excludes.
 * - `keyChanged`: runs after an upload fails with `key-epoch-changed`. It installs the current key before the retry.
 */
export interface TransportOptions {
    origin: string;
    token: string;
    ownerPublicKey: string;
    replica: {
        binding: SyncBinding;
        prepare(): Promise<SignedCheckpoint | null>;
        pending(): Promise<number>;
        acknowledge(operation: string, digest: string): Promise<void>;
        receive(envelope: SignedCheckpoint, author: Grant): Promise<string | null>;
    };
    status(state: ConnectionStatus, error?: string): void;
    projection(text: string): Promise<void> | void;
    fetch?: typeof fetch;
    environment?: Environment;
    streams?: Stream[];
    collectionScope?: boolean;
    directory?: boolean;
    known?(key: string): Promise<boolean>;
    wanted?(key: string): boolean;
    authorValid?(author: SignedGrant): Promise<boolean>;
    receive?(
        envelope: SignedCheckpoint,
        author: SignedGrant,
        signal: AbortSignal,
        directory?: SyncDirectory,
    ): Promise<string | null>;
    cycle?(signal: AbortSignal, directory?: SyncDirectory): Promise<void>;
    exclude?(): string[];
    keyChanged?(envelope: SignedCheckpoint, signal: AbortSignal): Promise<void>;
}
/**
 * Poll and upload loop of one scope. One poll runs at a time: upload the outbox, scan the checkpoint index, download
 * unseen checkpoints, then schedule the next poll. The loop runs only while it is started, the page is visible and
 * online, and no `pagehide` suspended it.
 */
export class Transport {
    /**
     * The latest error of a poll or an upload. `null` after `retry` or after an upload pass that emptied the outbox.
     */
    lastFailure: unknown = null;
    readonly options: TransportOptions;
    private enabled = false;
    // Incremented by each pause. A poll of an earlier generation stops at its next check.
    private generation = 0;
    // Abort handles of the running poll and of a running key refresh.
    private reads: AbortController | null = null;
    private keyRead: AbortController | null = null;
    private timer: ReturnType<typeof setTimeout> | undefined;
    private running = false;
    private sending: Promise<void> | null = null;
    // Index keys that this instance applied, found in durable progress, or rejected. The set is not durable.
    private seen = new Set<string>();
    private failures = 0;
    // True between `pagehide` and `pageshow`.
    private suspended = false;
    // Requests an immediate poll after the running one.
    private wake = false;
    private environment: Environment;
    // Visibility or connectivity changed: pause when unavailable, otherwise poll at once.
    private listener = () => {
        if (!this.available()) this.pause();
        else if (this.enabled) {
            this.wake = true;
            this.schedule(0);
        }
    };
    private pagehide = () => {
        this.suspended = true;
        this.pause();
    };
    private pageshow = () => {
        this.suspended = false;
        this.listener();
    };
    private available = () => !this.suspended && this.environment.visible() && this.environment.online();
    /**
     * Validates the origin and subscribes to page events. It sends nothing before `start`.
     * @throws `SyncError` `invalid-sync-origin` for an origin that is not a bare HTTPS or loopback HTTP origin.
     */
    constructor(options: TransportOptions) {
        const origin = new URL(options.origin);
        // The bearer token goes to this origin, so it must be a bare HTTPS origin. Loopback HTTP serves local
        // development.
        if (
            origin.username ||
            origin.password ||
            origin.search ||
            origin.hash ||
            origin.pathname !== '/' ||
            (origin.protocol !== 'https:' &&
                !(origin.protocol === 'http:' && ['localhost', '127.0.0.1'].includes(origin.hostname)))
        )
            throw new SyncError('invalid-sync-origin');
        this.options = options;
        this.environment = options.environment ?? {
            visible: () => !document.hidden,
            online: () => navigator.onLine,
            visibility: document,
            connectivity: window,
        };
        this.environment.visibility.addEventListener('visibilitychange', this.listener);
        this.environment.connectivity.addEventListener('online', this.listener);
        this.environment.connectivity.addEventListener('offline', this.listener);
        this.environment.connectivity.addEventListener('pagehide', this.pagehide);
        this.environment.connectivity.addEventListener('pageshow', this.pageshow);
    }
    /** Enables the loop and schedules an immediate poll. */
    start() {
        this.enabled = true;
        this.schedule(0);
    }
    /** Clears the failure state and starts the loop again, also after `attention` or `ended` stopped it. */
    retry() {
        this.failures = 0;
        this.lastFailure = null;
        this.enabled = true;
        this.schedule(0);
    }
    /** Aborts reads and the timer but keeps the enabled intent. A running upload request continues. */
    private pause() {
        this.generation++;
        this.reads?.abort();
        this.reads = null;
        this.keyRead?.abort();
        this.keyRead = null;
        clearTimeout(this.timer);
        this.timer = undefined;
        if (this.enabled && !this.environment.online()) this.options.status('offline');
    }
    /**
     * Disables the loop and aborts reads. An upload request in flight can still complete, but its acknowledgement is
     * ignored. The packet stays in the outbox, and a later upload sends the same bytes.
     */
    stop() {
        this.enabled = false;
        this.pause();
    }
    /** Stops the loop and removes the page event listeners. Call it before the instance is dropped. */
    dispose() {
        this.stop();
        this.environment.visibility.removeEventListener('visibilitychange', this.listener);
        for (const event of ['online', 'offline'])
            this.environment.connectivity.removeEventListener(event, this.listener);
        this.environment.connectivity.removeEventListener('pagehide', this.pagehide);
        this.environment.connectivity.removeEventListener('pageshow', this.pageshow);
    }
    /** Arms the next poll after `delay` milliseconds, unless a poll runs or the page is unavailable. */
    private schedule(delay: number) {
        clearTimeout(this.timer);
        if (!this.enabled || this.running || !this.available()) return;
        this.timer = setTimeout(() => {
            this.timer = undefined;
            void this.poll();
        }, delay);
    }
    /**
     * Sends one authenticated POST to `/sync/<action>` and returns the parsed JSON body. The body carries the
     * collection and the member of the binding. The token travels in the Authorization header. Cookies, caching,
     * redirects, and the referrer are off. The response is untrusted. Its size limit is `LIMITS.updateBytes` plus 32
     * KiB.
     * @returns Unvalidated JSON. The caller checks each field that it uses.
     * @throws `SyncError` with the error code of the service and the HTTP status for a failed request,
     *   `response-too-large` (413), or `invalid-service-response` (503) for a body that is not JSON.
     */
    async request(action: string, value: Record<string, unknown>, signal?: AbortSignal) {
        const started = performance.now();
        try {
            const b = this.options.replica.binding;
            const response = await (this.options.fetch ?? fetch)(new URL(`/sync/${action}`, this.options.origin), {
                method: 'POST',
                headers: { Authorization: `Bearer ${this.options.token}`, 'Content-Type': 'application/json' },
                body: JSON.stringify({ collection: b.collection, member: b.member, ...value }),
                signal,
                credentials: 'omit',
                cache: 'no-store',
                redirect: 'error',
                referrerPolicy: 'no-referrer',
                priority: 'low',
            });
            syncDebug.record('request.finished', {
                action,
                status: response.status,
                duration: Math.round(performance.now() - started),
            });
            // Read the stream in pieces, so the size limit applies before the complete body is in memory.
            const reader = response.body?.getReader(),
                chunks: Uint8Array[] = [];
            let size = 0;
            if (reader) {
                try {
                    while (true) {
                        const { done, value } = await reader.read();
                        if (done) break;
                        size += value.byteLength;
                        if (size > LIMITS.updateBytes + 32768) throw new SyncError('response-too-large', 413);
                        chunks.push(value);
                    }
                } finally {
                    await reader.cancel().catch(() => {});
                    reader.releaseLock();
                }
            }
            const bytes = new Uint8Array(size);
            let at = 0;
            for (const chunk of chunks) {
                bytes.set(chunk, at);
                at += chunk.length;
            }
            let result;
            try {
                result = JSON.parse(new TextDecoder().decode(bytes));
            } catch {
                throw new SyncError('invalid-service-response', 503);
            }
            if (!response.ok)
                throw new SyncError(
                    typeof result.error === 'string' ? result.error : 'sync-request-failed',
                    response.status,
                );
            return result;
        } catch (error) {
            syncDebug.record('request.failed', {
                action,
                duration: Math.round(performance.now() - started),
                ...safeError(error),
            });
            throw error;
        }
    }
    /**
     * Uploads queued packets in order until the outbox is empty or the transport becomes unavailable. Concurrent calls
     * share one run. The promise never rejects: a failure goes to `status` and `lastFailure`. A visibility change does
     * not abort a request in flight.
     */
    upload() {
        if (this.sending) return this.sending;
        if (!this.enabled || !this.available()) return Promise.resolve();
        this.sending = (async () => {
            let envelope: SignedCheckpoint | null = null;
            try {
                while (this.enabled && this.available() && (envelope = await this.options.replica.prepare())) {
                    if (!this.enabled || !this.available()) break;
                    this.options.status('working');
                    const result = await this.request('append', { envelope });
                    // A detached binding or a stopped session cannot resume uploads after this await.
                    if (!this.enabled) return;
                    // The service controls both values. The replica accepts them only when the digest matches its saved
                    // envelope.
                    await this.options.replica.acknowledge(result.operation, result.digest);
                }
                if (this.enabled) {
                    const pending = await this.options.replica.pending();
                    if (!pending) this.lastFailure = null;
                    this.options.status(pending ? 'waiting' : 'current');
                }
            } catch (error) {
                // The document key rotated while this packet waited. The host installs the current key, and the replica
                // seals the packet again.
                if (
                    error instanceof SyncError &&
                    error.code === 'key-epoch-changed' &&
                    envelope &&
                    this.options.keyChanged &&
                    this.enabled
                ) {
                    if (this.available()) {
                        const read = new AbortController();
                        this.keyRead = read;
                        try {
                            await this.options.keyChanged(envelope, read.signal);
                            if (this.enabled && this.available()) {
                                this.wake = true;
                                this.options.status('waiting');
                            }
                        } catch (failure) {
                            this.failure(failure);
                        } finally {
                            if (this.keyRead === read) this.keyRead = null;
                        }
                    }
                } else this.failure(error);
            } finally {
                this.sending = null;
            }
        })();
        return this.sending;
    }
    /**
     * Classifies an error and reports the state. 401, 403, and 410 end the connection. 400, 409, 413, and 507 need
     * attention. Both cases stop the loop. Any other error counts toward backoff. An abort is not a failure.
     */
    private failure(error: unknown) {
        if ((error as Error)?.name === 'AbortError') return;
        this.lastFailure = error;
        this.failures++;
        if (error instanceof SyncError && [401, 403, 410].includes(error.status)) {
            this.stop();
            this.options.status('ended', error.code);
        } else if (error instanceof SyncError && [400, 409, 413, 507].includes(error.status)) {
            this.stop();
            this.options.status('attention', error.code);
        } else this.options.status(this.environment.online() ? 'waiting' : 'offline', (error as Error)?.message);
    }
    /**
     * Runs one cycle: upload, then scan each index page and download each checkpoint that is neither seen nor known. It
     * does nothing while another poll runs or the transport is unavailable. It never rejects. Each cycle ends with the
     * schedule of the next poll.
     */
    async poll() {
        if (this.running || !this.enabled || !this.available()) return;
        this.wake = false;
        this.running = true;
        const generation = this.generation,
            reads = new AbortController();
        this.reads = reads;
        // False after a pause, a stop, or an abort. The poll checks it after each await, so a stale poll changes
        // nothing.
        const current = () => this.enabled && generation === this.generation && !reads.signal.aborted;
        try {
            await this.upload();
            let after: string | undefined;
            let directory: SyncDirectory | undefined;
            let downloads = 0,
                rejected = 0;
            const visited = new Set<string>();
            do {
                if (!current()) return;
                const page = await this.request(
                    'index',
                    {
                        document: this.options.collectionScope ? undefined : this.options.replica.binding.document,
                        after,
                        exclude: this.options.exclude?.(),
                        ...(this.options.directory && !after ? { directory: true } : {}),
                    },
                    reads.signal,
                );
                // The index is untrusted. Bound the page and require an explicit completion flag.
                if (
                    !Array.isArray(page.entries) ||
                    page.entries.length > LIMITS.pageSize ||
                    typeof page.complete !== 'boolean'
                )
                    throw new SyncError('invalid-checkpoint-index', 503);
                // The directory is unsigned service metadata. Check its size and each item before use.
                if (page.directory !== undefined) {
                    if (
                        !Array.isArray(page.directory?.documents) ||
                        page.directory.documents.length > LIMITS.documents ||
                        page.directory.documents.some(
                            (item: SyncDirectory['documents'][number]) =>
                                !validId(item?.id) ||
                                !['doc', 'folder'].includes(item.kind) ||
                                typeof item.deleted !== 'boolean' ||
                                !validId(item.creator),
                        )
                    )
                        throw new SyncError('invalid-checkpoint-index', 503);
                    directory = page.directory;
                }
                for (const entry of page.entries) {
                    if (!current()) return;
                    if (typeof entry.key !== 'string' || entry.key.length > 256)
                        throw new SyncError('invalid-checkpoint-key', 503);
                    // This adapter handles body state only. Reviews and manifests use
                    // their own typed projection adapters, never an opaque body update.
                    if (!(this.options.streams ?? ['body']).includes(entry.key.split('/').at(-2) as Stream)) continue;
                    visited.add(entry.key);
                    if (this.seen.has(entry.key)) continue;
                    if (this.options.wanted && !this.options.wanted(entry.key)) continue;
                    if (await this.options.known?.(entry.key)) {
                        this.seen.add(entry.key);
                        continue;
                    }
                    if (!current()) return;
                    const stored = await this.request('get', { key: entry.key }, reads.signal);
                    downloads++;
                    if (!current()) return;
                    // The service chooses the envelope and the author. Check both shapes, then require a grant that the
                    // owner key vouches for. A failure here is a service fault, not a bad packet, and ends the
                    // connection.
                    if (
                        !validGrant(stored.author) ||
                        !validCheckpoint(stored.envelope) ||
                        !(await (this.options.authorValid
                            ? this.options.authorValid(stored.author)
                            : verify(stored.author.grant, stored.author.signature, this.options.ownerPublicKey)))
                    )
                        throw new SyncError('invalid-author-grant', 403);
                    let text: string | null;
                    try {
                        text = await (this.options.receive
                            ? this.options.receive(stored.envelope, stored.author, reads.signal, directory)
                            : this.options.replica.receive(stored.envelope, stored.author.grant));
                    } catch (error) {
                        if (!(error instanceof RejectedPacket)) throw error;
                        // Any writer can upload a checkpoint that fails verification here. Skip it for
                        // this session, without progress, so one bad checkpoint cannot stop the connection.
                        this.seen.add(entry.key);
                        rejected++;
                        continue;
                    }
                    if (!current()) return;
                    if (text === null) continue;
                    await this.options.projection(text);
                    this.seen.add(entry.key);
                }
                if (page.complete) break;
                // A missing or repeated cursor would repeat the same page without end.
                if (typeof page.cursor !== 'string' || page.cursor === after)
                    throw new SyncError('invalid-checkpoint-cursor', 503);
                after = page.cursor;
            } while (current());
            // Forget keys that the index no longer lists, so the set stays bounded.
            this.seen = new Set([...this.seen].filter((key) => visited.has(key)));
            if (current()) await this.options.cycle?.(reads.signal, directory);
            if (current()) syncDebug.record('poll.finished', { items: visited.size, downloads, rejected });
            this.failures = 0;
        } catch (error) {
            this.failure(error);
        } finally {
            if (this.reads === reads) this.reads = null;
            this.running = false;
            // Visibility can change while an old request aborts. Intent remains enabled.
            // Next poll: at once after a wake, after 3 s normally, or after backoff. Backoff doubles from 2 s to a 60 s
            // limit, with 20% jitter.
            this.schedule(
                this.wake
                    ? 0
                    : this.failures
                      ? Math.min(60000, 1000 * 2 ** Math.min(6, this.failures)) * (0.8 + Math.random() * 0.4)
                      : 3000,
            );
        }
    }
}
