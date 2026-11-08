import { build } from 'esbuild';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { gzipSync } from 'node:zlib';

// Compile the standalone module graphs to source strings, with all closures resolved.
// Only these small runtimes are embedded; Paged.js remains an on-demand CDN dependency.
const runtimes = new Map();
const runtimePlugin = {
    name: 'standalone-runtimes',
    setup(builder) {
        builder.onResolve({ filter: /^mars-runtime:(reader|print)$/ }, ({ path }) => ({ path: path.split(':')[1], namespace: 'runtime' }));
        builder.onLoad({ filter: /.*/, namespace: 'runtime' }, async ({ path }) => {
            if (!runtimes.has(path)) runtimes.set(path, build({
                entryPoints: [`src/export/${path}-runtime.ts`], bundle: true, minify: true,
                format: 'iife', target: 'es2023', write: false, loader: { '.css': 'text' }, plugins: [runtimePlugin],
                legalComments: 'inline',
            }).then((result) => result.outputFiles[0].text));
            return { contents: `export default ${JSON.stringify(await runtimes.get(path))}`, loader: 'js' };
        });
    },
};

await build({
    entryPoints: ['src/main.ts'],
    bundle: true,
    minify: true,
    format: 'esm',
    target: 'es2023',
    outfile: 'dist/mars.js',
    loader: { '.css': 'text' },
    plugins: [runtimePlugin],
    legalComments: 'inline',
    logLevel: 'warning',
});

await build({ entryPoints: ['src/sync/responder.ts'], bundle: true, minify: true,
    format: 'iife', globalName: 'MARS_SYNC', target: 'es2023', outfile: 'dist/mars-sync-responder.js', legalComments: 'inline' });

// `--preview` writes dist/mars.html: the responder body as deploy.ts would serve it
// (bundles inlined, `{{TOOLS_HOST}}` / `{{MARS_HOST}}` substituted, not minified), for local testing.
if (process.argv.includes('--preview')) {
    const threeMf = '../three-mf/dist/three-mf.js';
    if (!existsSync(threeMf)) {
        if (!existsSync('../three-mf/node_modules')) execFileSync('npm', ['ci'], { cwd: '../three-mf', stdio: 'inherit' });
        execFileSync('npm', ['run', 'build'], { cwd: '../three-mf', stdio: 'inherit' });
    }
    const bundles = { mars: 'dist/mars.js', 'three-mf': threeMf };
    const html = readFileSync('../../mars.html', 'utf8')
        .replace(/\{\{\s*TOOLS_HOST\s*\}\}/g, process.env.SECUTILS_TOOLS_PUBLIC_HOST?.trim() || 'tools.secutils.dev')
        .replace(/\{\{\s*MARS_HOST\s*\}\}/g, process.env.SECUTILS_MARS_PUBLIC_HOST?.trim() || 'mars.secutils.dev')
        .replace(/(<script\b[^>]*\bdata-su-bundle="([a-z0-9_-]+)"[^>]*>)\s*<\/script>/gi,
            (_, open, name) => `${open}${gzipSync(readFileSync(bundles[name])).toString('base64')}</script>`);
    writeFileSync('dist/mars.html', html);
    console.log('dist/mars.html');
}
