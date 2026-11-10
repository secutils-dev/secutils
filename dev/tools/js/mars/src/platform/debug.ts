// Each feature owns its report schema and must return sanitized, allowlisted data.
import { featureFlags, registerFlagFeature, reportFlags } from './flags.ts';

interface DebugProvider { snapshot(): unknown; clear(): void }
const providers = new Map<string, DebugProvider>();
const tab = crypto.randomUUID().slice(0, 8);
export function registerDebugProvider(name: string, provider: DebugProvider) {
    if (!/^[a-z][a-z0-9_-]{0,63}$/.test(name)) throw new Error('Invalid diagnostic channel');
    registerFlagFeature(name); providers.set(name, provider);
}
export function clearDebugReport() { for (const provider of providers.values()) provider.clear(); }
export function debugReport() {
    const channels = Object.fromEntries([...providers].map(([name, provider]) => {
        try { return [name, provider.snapshot()]; } catch { return [name, { unavailable: true }]; }
    }));
    return JSON.stringify({ format: 2, build: '2026-11-10-reload-network-zoom-1', tab, flags: reportFlags(),
        environment: { browser: navigator.userAgent, online: navigator.onLine, hidden: document.hidden, secureContext: globalThis.isSecureContext, broadcastChannel: typeof BroadcastChannel !== 'undefined', webLocks: !!navigator.locks }, channels }, null, 2);
}
export function installDebug() {
    Object.assign(window, { marsDebug: { report: debugReport, clear: clearDebugReport } });
    let enabled = featureFlags.any('debug');
    featureFlags.onChange(() => { const next = featureFlags.any('debug'); if (enabled && !next) clearDebugReport(); enabled = next; });
}
