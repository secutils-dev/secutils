// Source pages reference shared files; deployed responders remain self-contained.
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const TOOLS_DIR = dirname(fileURLToPath(import.meta.url));
const SHARED_DIR = resolve(TOOLS_DIR, 'shared');

export function inlineSharedAssets(html: string): string {
    // Explicit opt-in: older tool pages keep their existing chrome.
    html = html.replace(/<!--[\s\S]*?-->|<link\b[^>]*\bdata-su-style="([a-z-]+)"[^>]*>/g, (tag, name: string) => {
        if (tag.startsWith('<!--')) return tag;
        if (!['tokens', 'components'].includes(name)) throw new Error(`Unknown data-su-style: ${name}`);
        if (!tag.includes(`href="./shared/${name}.css"`) || !tag.includes('rel="stylesheet"')) {
            throw new Error(`data-su-style="${name}" must reference ./shared/${name}.css`);
        }
        const css = readFileSync(resolve(SHARED_DIR, `${name}.css`), 'utf8');
        if (/<\/style/i.test(css)) throw new Error(`Shared stylesheet ${name} contains </style`);
        return `<style data-su-style="${name}">${css}</style>`;
    });

    html = html.replace(/<!--[\s\S]*?-->|<img\b[^>]*\bdata-su-app-icon="([a-z0-9_-]+)"[^>]*>/g, (tag, name: string) => {
        if (tag.startsWith('<!--')) return tag;
        const path = `./${name}/assets/${name}-app-icon.svg`;
        if (!tag.includes(`src="${path}"`)) throw new Error(`data-su-app-icon="${name}" must reference ${path}`);
        const svg = readFileSync(resolve(TOOLS_DIR, name, 'assets', `${name}-app-icon.svg`));
        return tag.replace(`src="${path}"`, `src="data:image/svg+xml;base64,${svg.toString('base64')}"`).replace(/\s+data-su-app-icon="[^"]*"/, '');
    });

    // A data URL is an ordinary ES module, with its own scope and no HTTP fetch.
    // Restrict rewriting to module script bodies, never user text or responders.
    return html.replace(/<!--[\s\S]*?-->|(<script\b[^>]*\btype="module"[^>]*>)([\s\S]*?)(<\/script>)/g, (tag, open, source, close) => {
        if (tag.startsWith('<!--')) return tag;
        const code = source.replace(/(\bfrom\s*)['"]\.\/shared\/icons\.js['"]/g, (_: string, from: string) => {
            const icons = readFileSync(resolve(SHARED_DIR, 'icons.js'), 'utf8');
            return `${from}'data:text/javascript;base64,${Buffer.from(icons).toString('base64')}'`;
        });
        return `${open}${code}${close}`;
    });
}
