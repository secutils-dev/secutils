// Dependency-free bundle assembly for deployment, prebuilds, and app previews.
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { discoverBundles, TOOLS_DIR } from './sources.ts';

function newestMtimeUnder(root: string, excludeDirs: Set<string>): number {
    let newest = 0;
    const stack: string[] = [root];
    while (stack.length > 0) {
        const dir = stack.pop()!;
        let entries: string[];
        try { entries = readdirSync(dir); }
        catch { continue; }
        for (const entry of entries) {
            const full = join(dir, entry);
            let st;
            try { st = statSync(full); }
            catch { continue; }
            if (st.isDirectory()) {
                if (!excludeDirs.has(entry)) stack.push(full);
            } else if (st.mtimeMs > newest) newest = st.mtimeMs;
        }
    }
    return newest;
}

// Reuse shared bundles across pages in one assembly run.
const cache = new Map<string, string>();

export function ensureBundleBuilt(name: string): string {
    if (!/^[a-z0-9_-]+$/.test(name)) throw new Error(`invalid data-su-bundle name "${name}"`);
    const cached = cache.get(name);
    if (cached !== undefined) return cached;
    const bundleDir = discoverBundles().find((bundle) => bundle.name === name)?.directory;
    if (!bundleDir) throw new Error(`bundle "${name}" not found: declare package.json#suBundle in an app or shared/libs package`);
    const distPath = resolve(bundleDir, 'dist', `${name}.js`);
    const exclude = new Set(['dist', 'node_modules']);
    // MARS imports the shared chrome and glyphs from outside its package.
    const sourceMtime = Math.max(newestMtimeUnder(bundleDir, exclude),
        name === 'mars' ? newestMtimeUnder(resolve(TOOLS_DIR, 'shared'), exclude) : 0);
    if (!existsSync(distPath) || sourceMtime > statSync(distPath).mtimeMs) {
        console.log(`tools-bundles: building ${name}`);
        if (!existsSync(resolve(bundleDir, 'node_modules'))) {
            execFileSync('npm', ['ci'], { cwd: bundleDir, stdio: 'inherit' });
        }
        execFileSync('npm', ['run', 'build'], { cwd: bundleDir, stdio: 'inherit' });
        if (!existsSync(distPath)) throw new Error(`bundle "${name}" build finished but ${distPath} was not produced`);
    }
    const code = readFileSync(distPath, 'utf8');
    cache.set(name, code);
    return code;
}
