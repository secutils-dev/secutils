// Source layout is independent of public URLs and responder IDs.
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const TOOLS_DIR = resolve(dirname(fileURLToPath(import.meta.url)), '..');

export interface ToolSource {
    slug: string;
    // Paths relative to TOOLS_DIR, never deployment filenames.
    html: string;
    skill?: string;
}

export function discoverTools(root = TOOLS_DIR): ToolSource[] {
    const tools: ToolSource[] = [];
    for (const entry of readdirSync(root, { withFileTypes: true })) {
        if (entry.name === 'shared' || entry.name.startsWith('.')) continue;
        if (entry.isFile() && entry.name.endsWith('.skill.md')) {
            throw new Error(`Move ${entry.name} and its app into an app directory with index.html and SKILL.md`);
        }
        if (entry.isFile() && entry.name.endsWith('.html')) {
            const slug = basename(entry.name, '.html');
            tools.push({ slug, html: entry.name });
        } else if (entry.isDirectory() && existsSync(join(root, entry.name, 'index.html'))) {
            const skill = join(entry.name, 'SKILL.md');
            tools.push({ slug: entry.name, html: join(entry.name, 'index.html'),
                ...(existsSync(join(root, skill)) ? { skill } : {}) });
        }
    }
    const seen = new Set<string>();
    for (const tool of tools) {
        if (seen.has(tool.slug)) throw new Error(`Duplicate tool source: ${tool.slug}`);
        seen.add(tool.slug);
    }
    return tools.sort((a, b) => a.slug.localeCompare(b.slug, 'en'));
}

export interface BundleSource {
    name: string;
    directory: string;
}

export function discoverBundles(root = TOOLS_DIR): BundleSource[] {
    const shared = join(root, 'shared/libs');
    const directories = [
        ...discoverTools(root).filter((tool) => tool.html.includes('/')).map((tool) => join(root, tool.slug)),
        ...(existsSync(shared) ? readdirSync(shared, { withFileTypes: true })
            .filter((entry) => entry.isDirectory()).map((entry) => join(shared, entry.name)) : []),
    ];
    const bundles: BundleSource[] = [];
    for (const directory of directories) {
        const file = join(directory, 'package.json');
        if (!existsSync(file)) continue;
        const { suBundle: name } = JSON.parse(readFileSync(file, 'utf8'));
        if (typeof name !== 'string' || !/^[a-z0-9_-]+$/.test(name)) throw new Error(`Missing or invalid suBundle in ${file}`);
        if (bundles.some((bundle) => bundle.name === name)) throw new Error(`Duplicate bundle name: ${name}`);
        bundles.push({ name, directory });
    }
    return bundles.sort((a, b) => a.name.localeCompare(b.name, 'en'));
}
