// Build script (`npm run build`). It writes `dist/mars.js`, the app bundle that deployment
// inlines into `index.html`, and `dist/mars-sync-responder.js`, the Sync part of the responder.
// `--preview` also writes `dist/mars.html` for the local server.
// License notices stay in the output (`legalComments: 'inline'`, the editor notice block, and
// the tracker banner). A bundled editor package without a license file fails the build.
import { build } from 'esbuild';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { gzipSync } from 'node:zlib';

// Compile separate module graphs to embedded source strings. The editor remains
// lazy-imported; Paged.js remains an on-demand CDN dependency.
// name -> promise of the compiled source. Each runtime builds once, also when imported twice.
const runtimes = new Map();
const runtimePlugin = {
    name: 'standalone-runtimes',
    setup(builder) {
        // `import source from 'mars-runtime:<name>'` gives the compiled runtime as a string.
        builder.onResolve({ filter: /^mars-runtime:(reader|print|editor)$/ }, ({ path }) => ({
            path: path.split(':')[1],
            namespace: 'runtime',
        }));
        builder.onLoad({ filter: /.*/, namespace: 'runtime' }, async ({ path }) => {
            if (!runtimes.has(path))
                runtimes.set(
                    path,
                    build({
                        // The reader and print runtimes are classic scripts for exported pages. The editor is an ES
                        // module that the app loads from a Blob URL.
                        entryPoints: [`src/${path === 'editor' ? 'doc' : 'export'}/${path}-runtime.ts`],
                        bundle: true,
                        minify: true,
                        format: path === 'editor' ? 'esm' : 'iife',
                        target: 'es2023',
                        write: false,
                        loader: { '.css': 'text' },
                        plugins: [runtimePlugin],
                        legalComments: 'inline',
                        metafile: path === 'editor',
                    }).then((result) => {
                        const text = result.outputFiles[0].text;
                        if (path !== 'editor') return text;
                        // The editor bundles npm packages. Collect them from the metafile and add each license text.
                        // Packages with the same license text share one notice.
                        const packages = [
                            ...new Set(
                                Object.keys(result.metafile.inputs).flatMap(
                                    (input) => input.match(/^node_modules\/((?:@[^/]+\/)?[^/]+)\//)?.[1] ?? [],
                                ),
                            ),
                        ].sort();
                        const notices = new Map();
                        for (const name of packages) {
                            const file = ['LICENSE', 'LICENSE.txt', 'LICENSE.md']
                                .map((file) => `node_modules/${name}/${file}`)
                                .find(existsSync);
                            if (!file) throw new Error(`Missing bundled-editor license: ${name}`);
                            const license = readFileSync(file, 'utf8').trim();
                            notices.set(license, [...(notices.get(license) ?? []), name]);
                        }
                        return `/*!\n${[...notices].map(([license, names]) => `${names.join(', ')}\n${license}`).join('\n\n')}\n*/\n${text}`;
                    }),
                );
            return { contents: `export default ${JSON.stringify(await runtimes.get(path))}`, loader: 'js' };
        });
    },
};

// The app bundle. CSS files load as text, and `boot.ts` injects them.
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
    banner: {
        js: `/*! @plausible-analytics/tracker\n${readFileSync('node_modules/@plausible-analytics/tracker/LICENSE.md', 'utf8').trim()}\n*/`,
    },
    logLevel: 'warning',
});

// The Sync responder, as a script that defines the global `MARS_SYNC`. `sync-assets.ts`
// inlines it into the responder script of `index.html`.
await build({
    entryPoints: ['src/sync/responder.ts'],
    bundle: true,
    minify: true,
    format: 'iife',
    globalName: 'MARS_SYNC',
    target: 'es2023',
    outfile: 'dist/mars-sync-responder.js',
    legalComments: 'inline',
});

// `--preview` writes dist/mars.html: the responder body as deploy.ts would serve it
// (bundles inlined, `{{TOOLS_HOST}}` / `{{MARS_HOST}}` substituted, not minified), for local testing.
if (process.argv.includes('--preview')) {
    const { ensureBundleBuilt } = await import('../shared/bundles.ts');
    const bundles = { mars: readFileSync('dist/mars.js'), 'three-mf': Buffer.from(ensureBundleBuilt('three-mf')) };
    const html = readFileSync('index.html', 'utf8')
        .replace(/\{\{\s*TOOLS_HOST\s*\}\}/g, process.env.SECUTILS_TOOLS_PUBLIC_HOST?.trim() || 'tools.secutils.dev')
        .replace(/\{\{\s*MARS_HOST\s*\}\}/g, process.env.SECUTILS_MARS_PUBLIC_HOST?.trim() || 'mars.secutils.dev')
        .replace(
            /(<script\b[^>]*\bdata-su-bundle="([a-z0-9_-]+)"[^>]*>)\s*<\/script>/gi,
            (_, open, name) => `${open}${gzipSync(bundles[name]).toString('base64')}</script>`,
        );
    writeFileSync('dist/mars.html', html);
    console.log('dist/mars.html');
}
