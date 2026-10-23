import { build } from 'esbuild';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { gzipSync } from 'node:zlib';

await build({
    entryPoints: ['src/main.ts'],
    bundle: true,
    minify: true,
    format: 'esm',
    target: 'es2023',
    outfile: 'dist/mars.js',
    loader: { '.css': 'text' },
    legalComments: 'inline',
    logLevel: 'warning',
});

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
