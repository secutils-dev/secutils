// The debug report: a registry of diagnostic channels and the assembly of their snapshots.
// Each feature owns its report schema and must return sanitized, allowlisted data.
// This module itself adds only the build label, a random tab label, the flags, and browser
// capabilities. It never sends a report anywhere: the reader copies it from Developer tools
// or from the console.
import { featureFlags, registerFlagFeature, reportFlags } from './flags.ts';

// One diagnostic channel. `snapshot` returns JSON-serializable, allowlisted data. `clear`
// discards the captured data.
interface DebugProvider {
    snapshot(): unknown;
    clear(): void;
}
const providers = new Map<string, DebugProvider>();
// A random label of this tab, new on each load. It tells the reports of two tabs apart.
const tab = crypto.randomUUID().slice(0, 8);
/**
 * Registers a diagnostic channel and its flag feature (`debug:<name>`).
 * Throws for a name outside the flag grammar. A second registration replaces the first.
 */
export function registerDebugProvider(name: string, provider: DebugProvider) {
    if (!/^[a-z][a-z0-9_-]{0,63}$/.test(name)) throw new Error('Invalid diagnostic channel');
    registerFlagFeature(name);
    providers.set(name, provider);
}
/** Discards the captured data of every channel. */
export function clearDebugReport() {
    for (const provider of providers.values()) provider.clear();
}
/**
 * Returns the report as formatted JSON: format version, build label, tab label, flags,
 * browser capabilities, and one snapshot for each channel. A channel whose snapshot throws
 * reports `{ unavailable: true }`.
 */
export function debugReport() {
    const channels = Object.fromEntries(
        [...providers].map(([name, provider]) => {
            try {
                return [name, provider.snapshot()];
            } catch {
                return [name, { unavailable: true }];
            }
        }),
    );
    return JSON.stringify(
        {
            format: 2,
            build: '2026-11-10-reload-network-zoom-1',
            tab,
            flags: reportFlags(),
            environment: {
                browser: navigator.userAgent,
                online: navigator.onLine,
                hidden: document.hidden,
                secureContext: globalThis.isSecureContext,
                broadcastChannel: typeof BroadcastChannel !== 'undefined',
                webLocks: !!navigator.locks,
            },
            channels,
        },
        null,
        2,
    );
}
/**
 * Exposes the `marsDebug` console API. Captured data is discarded when the last debug flag
 * turns off, in this tab or in another one.
 */
export function installDebug() {
    Object.assign(window, { marsDebug: { report: debugReport, clear: clearDebugReport } });
    let enabled = featureFlags.any('debug');
    featureFlags.onChange(() => {
        const next = featureFlags.any('debug');
        if (enabled && !next) clearDebugReport();
        enabled = next;
    });
}
