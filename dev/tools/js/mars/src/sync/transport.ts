// Short, scoped polls. Visibility aborts reads, not an upload already in progress.
// Credentials travel only in Authorization headers. Correctness does not use pagehide.
import { verify } from './crypto.ts';
import { LIMITS, SyncError, validCheckpoint, validGrant, type SignedCheckpoint } from './protocol.ts';
import type { Grant, SignedGrant, Stream, SyncBinding } from './protocol.ts';

export type ConnectionStatus = 'waiting' | 'working' | 'current' | 'offline' | 'attention' | 'ended';
interface Environment {
    visible(): boolean;
    online(): boolean;
    visibility: EventTarget;
    connectivity: EventTarget;
}
export interface TransportOptions {
    origin: string; token: string; ownerPublicKey: string; replica: {
        binding: SyncBinding; prepare(): Promise<SignedCheckpoint | null>; pending(): Promise<number>;
        acknowledge(operation: string, digest: string): Promise<void>; receive(envelope: SignedCheckpoint, author: Grant): Promise<string | null>;
    };
    status(state: ConnectionStatus, error?: string): void;
    projection(text: string): Promise<void> | void;
    fetch?: typeof fetch;
    environment?: Environment;
    streams?: Stream[];
    collectionScope?: boolean;
    authorValid?(author: SignedGrant): Promise<boolean>;
    receive?(envelope: SignedCheckpoint, author: SignedGrant, signal: AbortSignal): Promise<string | null>;
    cycle?(signal: AbortSignal): Promise<void>;
    exclude?(): string[];
    keyChanged?(envelope: SignedCheckpoint, signal: AbortSignal): Promise<void>;
}
export class Transport {
    readonly options: TransportOptions;
    private enabled = false;
    private generation = 0;
    private reads: AbortController | null = null;
    private keyRead: AbortController | null = null;
    private timer: ReturnType<typeof setTimeout> | undefined;
    private running = false;
    private sending: Promise<void> | null = null;
    private seen = new Set<string>();
    private failures = 0;
    private suspended = false;
    private wake = false;
    private environment: Environment;
    private listener = () => {
        if (!this.available()) this.pause();
        else if (this.enabled) { this.wake = true; this.schedule(0); }
    };
    private pagehide = () => { this.suspended = true; this.pause(); };
    private pageshow = () => { this.suspended = false; this.listener(); };
    private available = () => !this.suspended && this.environment.visible() && this.environment.online();
    constructor(options: TransportOptions) {
        const origin = new URL(options.origin);
        if (origin.username || origin.password || origin.search || origin.hash || origin.pathname !== '/'
            || (origin.protocol !== 'https:' && !(origin.protocol === 'http:' && ['localhost', '127.0.0.1'].includes(origin.hostname)))) throw new SyncError('invalid-sync-origin');
        this.options = options;
        this.environment = options.environment ?? { visible: () => !document.hidden, online: () => navigator.onLine,
            visibility: document, connectivity: window };
        this.environment.visibility.addEventListener('visibilitychange', this.listener);
        this.environment.connectivity.addEventListener('online', this.listener);
        this.environment.connectivity.addEventListener('offline', this.listener);
        this.environment.connectivity.addEventListener('pagehide', this.pagehide);
        this.environment.connectivity.addEventListener('pageshow', this.pageshow);
    }
    start() { this.enabled = true; this.schedule(0); }
    retry() { this.failures = 0; this.enabled = true; this.schedule(0); }
    private pause() {
        this.generation++; this.reads?.abort(); this.reads = null; this.keyRead?.abort(); this.keyRead = null;
        clearTimeout(this.timer); this.timer = undefined;
        if (this.enabled && !this.environment.online()) this.options.status('offline');
    }
    stop() { this.enabled = false; this.pause(); }
    dispose() {
        this.stop();
        this.environment.visibility.removeEventListener('visibilitychange', this.listener);
        for (const event of ['online', 'offline']) this.environment.connectivity.removeEventListener(event, this.listener);
        this.environment.connectivity.removeEventListener('pagehide', this.pagehide);
        this.environment.connectivity.removeEventListener('pageshow', this.pageshow);
    }
    private schedule(delay: number) {
        clearTimeout(this.timer);
        if (!this.enabled || this.running || !this.available()) return;
        this.timer = setTimeout(() => { this.timer = undefined; void this.poll(); }, delay);
    }
    async request(action: string, value: Record<string, unknown>, signal?: AbortSignal) {
        const b = this.options.replica.binding;
        const response = await (this.options.fetch ?? fetch)(new URL(`/sync/${action}`, this.options.origin), {
            method: 'POST', headers: { Authorization: `Bearer ${this.options.token}`, 'Content-Type': 'application/json' },
            body: JSON.stringify({ collection: b.collection, member: b.member, ...value }), signal,
            credentials: 'omit', cache: 'no-store', redirect: 'error', referrerPolicy: 'no-referrer', priority: 'low',
        });
        const reader = response.body?.getReader(), chunks: Uint8Array[] = [];
        let size = 0;
        if (reader) {
            try {
                while (true) {
                    const { done, value } = await reader.read(); if (done) break;
                    size += value.byteLength;
                    if (size > LIMITS.updateBytes + 32768) throw new SyncError('response-too-large', 413);
                    chunks.push(value);
                }
            } finally { await reader.cancel().catch(() => {}); reader.releaseLock(); }
        }
        const bytes = new Uint8Array(size); let at = 0;
        for (const chunk of chunks) { bytes.set(chunk, at); at += chunk.length; }
        let result;
        try { result = JSON.parse(new TextDecoder().decode(bytes)); } catch { throw new SyncError('invalid-service-response', 503); }
        if (!response.ok) throw new SyncError(typeof result.error === 'string' ? result.error : 'sync-request-failed', response.status);
        return result;
    }
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
                    await this.options.replica.acknowledge(result.operation, result.digest);
                }
                if (this.enabled) this.options.status(await this.options.replica.pending() ? 'waiting' : 'current');
            } catch (error) {
                if (error instanceof SyncError && error.code === 'key-epoch-changed' && envelope && this.options.keyChanged && this.enabled) {
                    if (this.available()) {
                        const read = new AbortController(); this.keyRead = read;
                        try { await this.options.keyChanged(envelope, read.signal); if (this.enabled && this.available()) { this.wake = true; this.options.status('waiting'); } }
                        catch (failure) { this.failure(failure); }
                        finally { if (this.keyRead === read) this.keyRead = null; }
                    }
                } else this.failure(error);
            }
            finally { this.sending = null; }
        })();
        return this.sending;
    }
    private failure(error: unknown) {
        if ((error as Error)?.name === 'AbortError') return;
        this.failures++;
        if (error instanceof SyncError && [401, 403, 410].includes(error.status)) { this.stop(); this.options.status('ended', error.code); }
        else if (error instanceof SyncError && [400, 409, 413, 507].includes(error.status)) { this.stop(); this.options.status('attention', error.code); }
        else this.options.status(this.environment.online() ? 'waiting' : 'offline', (error as Error)?.message);
    }
    async poll() {
        if (this.running || !this.enabled || !this.available()) return;
        this.wake = false;
        this.running = true;
        const generation = this.generation, reads = new AbortController(); this.reads = reads;
        const current = () => this.enabled && generation === this.generation && !reads.signal.aborted;
        try {
            await this.upload();
            let after: string | undefined;
            const visited = new Set<string>();
            do {
                if (!current()) return;
                const page = await this.request('index', { document: this.options.collectionScope ? undefined : this.options.replica.binding.document, after, exclude: this.options.exclude?.() }, reads.signal);
                if (!Array.isArray(page.entries) || page.entries.length > LIMITS.pageSize || typeof page.complete !== 'boolean') throw new SyncError('invalid-checkpoint-index', 503);
                for (const entry of page.entries) {
                    if (!current()) return;
                    if (typeof entry.key !== 'string' || entry.key.length > 256) throw new SyncError('invalid-checkpoint-key', 503);
                    // This adapter handles body state only. Reviews and manifests use
                    // their own typed projection adapters, never an opaque body update.
                    if (!(this.options.streams ?? ['body']).includes(entry.key.split('/').at(-2) as Stream)) continue;
                    visited.add(entry.key);
                    if (this.seen.has(entry.key)) continue;
                    const stored = await this.request('get', { key: entry.key }, reads.signal);
                    if (!current()) return;
                    if (!validGrant(stored.author) || !validCheckpoint(stored.envelope)
                        || !(await (this.options.authorValid ? this.options.authorValid(stored.author) : verify(stored.author.grant, stored.author.signature, this.options.ownerPublicKey)))) throw new SyncError('invalid-author-grant', 403);
                    const text = await (this.options.receive ? this.options.receive(stored.envelope, stored.author, reads.signal) : this.options.replica.receive(stored.envelope, stored.author.grant));
                    if (!current()) return;
                    if (text === null) continue;
                    await this.options.projection(text);
                    this.seen.add(entry.key);
                }
                if (page.complete) break;
                if (typeof page.cursor !== 'string' || page.cursor === after) throw new SyncError('invalid-checkpoint-cursor', 503);
                after = page.cursor;
            } while (current());
            this.seen = new Set([...this.seen].filter((key) => visited.has(key)));
            if (current()) await this.options.cycle?.(reads.signal);
            this.failures = 0;
        } catch (error) { this.failure(error); }
        finally {
            if (this.reads === reads) this.reads = null;
            this.running = false;
            // Visibility can change while an old request aborts. Intent remains enabled.
            this.schedule(this.wake ? 0 : this.failures ? Math.min(60000, 1000 * 2 ** Math.min(6, this.failures)) * (0.8 + Math.random() * 0.4) : 3000);
        }
    }
}
