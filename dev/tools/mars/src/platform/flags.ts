// Profile-wide client flags. They are opt-ins, never server authorization.
//
// Owns the flag grammar, the stored rule set (`localStorage`, shared by every tab of the
// profile), and the URL controls `?flags=`, `?debug=`, and `?experimental=`.
// A rule is `<kind>` or `<kind>:<feature>`, with a leading `-` for an opt-out. A feature rule
// wins over its category rule. Flag text from the URL and from storage is untrusted: only
// tokens that match the grammar are kept.

/** The flag categories. */
export type FlagKind = 'debug' | 'experimental';
const KINDS: FlagKind[] = ['debug', 'experimental'];
// One token: an optional `-`, a category, and an optional feature name of 1 to 64 characters.
const TOKEN = /^(-?)(debug|experimental)(?::([a-z][a-z0-9_-]{0,63}))?$/;
/**
 * Parses a comma-separated flag list into rules (rule name -> enabled). A later token replaces
 * an earlier one with the same name. Invalid tokens are dropped.
 * It reads at most 4096 characters and 100 tokens, because the input can come from a URL.
 */
export function parseFlags(value: string) {
    const rules = new Map<string, boolean>();
    for (const token of value.slice(0, 4096).split(',').slice(0, 100)) {
        const match = TOKEN.exec(token.trim().toLowerCase());
        if (match) rules.set(`${match[2]}${match[3] ? `:${match[3]}` : ''}`, match[1] !== '-');
    }
    return rules;
}
// The rules as sorted tokens. An opt-out has a leading `-`.
const serialize = (rules: Map<string, boolean>) =>
    [...rules].sort(([a], [b]) => a.localeCompare(b)).map(([key, enabled]) => `${enabled ? '' : '-'}${key}`);
/** True when the feature rule, or else the category rule, enables the flag. The default is false. */
export function flagEnabled(rules: Map<string, boolean>, kind: FlagKind, feature?: string) {
    return (feature ? rules.get(`${kind}:${feature}`) : undefined) ?? rules.get(kind) ?? false;
}
// The I/O of `FeatureFlags`. The browser binding is `featureFlags` below, and tests supply their own.
interface FlagEnvironment {
    // The saved flag list, or null when none was ever saved.
    read(): string | null;
    write(value: string): void;
    // True when the retired `mars.debug` marker was set. The call also removes the marker.
    legacy(): boolean;
    // The query parameters of the page address.
    query(): URLSearchParams;
    // Removes the flag control parameters from the address.
    consume(): void;
    // Reports a change of the saved list by another tab.
    listen?(changed: (value: string | null) => void): void;
}
/**
 * The flag rules of this profile. Configuration is lazy: the first call of a public method
 * reads storage and the URL, so an import has no side effect. Each change is saved and then
 * announced to the listeners. A storage failure is ignored, and the flags then last for this tab.
 */
export class FeatureFlags {
    private configured = false;
    private rules = new Map<string, boolean>();
    private listeners = new Set<() => void>();
    private environment: FlagEnvironment;
    constructor(environment: FlagEnvironment) {
        this.environment = environment;
    }
    // Order of sources: the saved list, then the retired debug marker (only when nothing is saved),
    // then the URL. `?flags=` replaces every rule. `?debug=` and `?experimental=` replace one category.
    private configure() {
        if (this.configured) return;
        this.configured = true;
        let saved: string | null = null,
            legacy = false;
        try {
            saved = this.environment.read();
        } catch {}
        try {
            legacy = this.environment.legacy();
        } catch {}
        this.rules = parseFlags(saved ?? '');
        if (saved === null && legacy) this.rules.set('debug:sync', true);
        const query = this.environment.query();
        if (query.has('flags')) this.rules = parseFlags(query.getAll('flags').join(','));
        else
            for (const kind of KINDS)
                if (query.has(kind)) {
                    this.removeKind(kind);
                    const values = query
                        .getAll(kind)
                        .flatMap((value) => value.split(','))
                        .map((value) => value.trim().toLowerCase());
                    // `off`, `0`, or `false` clears the category. `1`, `true`, `all`, or an empty value enables
                    // the complete category. Every other value names a feature.
                    if (!values.some((value) => ['off', '0', 'false'].includes(value))) {
                        const parsed = parseFlags(
                            values
                                .map((value) =>
                                    ['1', 'true', 'all'].includes(value) || value === '' ? kind : `${kind}:${value}`,
                                )
                                .join(','),
                        );
                        for (const [key, value] of parsed) this.rules.set(key, value);
                    }
                }
        // Consume control parameters so a later reload cannot undo an explicit Off.
        if (query.has('flags') || KINDS.some((kind) => query.has(kind))) this.environment.consume();
        this.persist();
        // Another tab changed the saved list. Adopt it without a second write.
        this.environment.listen?.((value) => {
            this.rules = parseFlags(value ?? '');
            this.notify();
        });
    }
    private removeKind(kind: FlagKind) {
        for (const key of this.rules.keys()) if (key === kind || key.startsWith(`${kind}:`)) this.rules.delete(key);
    }
    private persist() {
        try {
            this.environment.write(serialize(this.rules).join(','));
        } catch {}
    }
    private notify() {
        for (const listener of this.listeners) listener();
    }
    /** The active rules as sorted tokens. */
    list() {
        this.configure();
        return serialize(this.rules);
    }
    /** True when `kind` is on for `feature`, or for the category when `feature` is omitted. */
    enabled(kind: FlagKind, feature?: string) {
        this.configure();
        return flagEnabled(this.rules, kind, feature);
    }
    /** True when the category is on or at least one of its features is on. */
    any(kind: FlagKind) {
        this.configure();
        return this.enabled(kind) || [...this.rules].some(([key, value]) => value && key.startsWith(`${kind}:`));
    }
    /** Replaces every rule with the parsed list. */
    set(value: string | string[]) {
        this.configure();
        this.rules = parseFlags(Array.isArray(value) ? value.join(',') : value);
        this.persist();
        this.notify();
    }
    /** Enables a feature, or the complete category. The category form also removes its feature rules. */
    enable(kind: FlagKind, feature?: string) {
        this.configure();
        if (!feature) this.removeKind(kind);
        const parsed = parseFlags(`${kind}${feature ? `:${feature}` : ''}`);
        for (const [key, value] of parsed) this.rules.set(key, value);
        this.persist();
        this.notify();
    }
    /**
     * Disables a feature, or the complete category with its feature rules.
     * A feature under an enabled category gets an explicit opt-out rule. An invalid feature
     * name has no effect.
     */
    disable(kind: FlagKind, feature?: string) {
        this.configure();
        if (!feature) this.removeKind(kind);
        else {
            const key = `${kind}:${feature}`;
            if (!TOKEN.test(key)) return;
            this.rules.delete(key);
            if (this.rules.get(kind)) this.rules.set(key, false);
        }
        this.persist();
        this.notify();
    }
    /** Removes every rule. */
    clear() {
        this.set('');
    }
    /** Registers a change listener and returns a function that removes it. */
    onChange(listener: () => void) {
        this.configure();
        this.listeners.add(listener);
        return () => this.listeners.delete(listener);
    }
}
// `localStorage` key of the saved flag list: comma-separated tokens, '' for no flags.
const STORAGE_KEY = 'mars.flags';
/** The flags of this browser profile. In Node, without `localStorage` and `location`, it starts empty. */
export const featureFlags = new FeatureFlags({
    read: () => (typeof localStorage === 'undefined' ? null : localStorage.getItem(STORAGE_KEY)),
    // Keep an empty marker: another tab's old session flag must not resurrect debug.
    write: (value) => localStorage.setItem(STORAGE_KEY, value),
    // The retired marker `mars.debug` (`1` or `sync`) in local or session storage enabled Sync debug.
    legacy: () => {
        let enabled = false;
        for (const name of ['localStorage', 'sessionStorage'] as const)
            try {
                const storage = globalThis[name];
                enabled ||= ['1', 'sync'].includes(storage?.getItem('mars.debug') ?? '');
                storage?.removeItem('mars.debug');
            } catch {}
        return enabled;
    },
    query: () => (typeof location === 'undefined' ? new URLSearchParams() : new URL(location.href).searchParams),
    consume: () => {
        if (typeof history === 'undefined') return;
        const url = new URL(location.href);
        for (const key of ['flags', ...KINDS]) url.searchParams.delete(key);
        history.replaceState(history.state, '', url.href);
    },
    listen: (changed) => {
        if (typeof window !== 'undefined')
            window.addEventListener('storage', (event) => {
                if (event.key === STORAGE_KEY || event.key === null) changed(event.newValue);
            });
    },
});
// Feature names that a module registered. Only these appear as scoped flags in a debug report.
const features = new Set<string>();
/** Registers a feature name for debug reports. A name outside the grammar is ignored. */
export const registerFlagFeature = (name: string) => {
    if (/^[a-z][a-z0-9_-]{0,63}$/.test(name)) features.add(name);
};
/**
 * The flags for a debug report: category rules, and feature rules of registered features.
 * A feature name that a reader typed and that no module registered stays out of the report.
 */
export const reportFlags = () =>
    featureFlags.list().filter((token) => !token.includes(':') || features.has(token.split(':')[1]));
/** Reads the flags, which consumes the URL controls, and exposes the `marsFlags` console API. */
export function installFlags() {
    featureFlags.list();
    Object.assign(window, {
        marsFlags: {
            list: () => featureFlags.list(),
            enabled: (kind: FlagKind, feature?: string) => featureFlags.enabled(kind, feature),
            set: (value: string | string[]) => featureFlags.set(value),
            enable: (kind: FlagKind, feature?: string) => featureFlags.enable(kind, feature),
            disable: (kind: FlagKind, feature?: string) => featureFlags.disable(kind, feature),
            clear: () => featureFlags.clear(),
        },
    });
}
