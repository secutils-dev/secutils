// Embed the browser JWT API and its license in one module, without CDN imports.
import { build } from 'esbuild';
import { readFileSync } from 'node:fs';

const license = readFileSync('node_modules/jose/LICENSE.md', 'utf8').trim();
await build({
    entryPoints: ['src/entry.js'],
    bundle: true,
    minify: true,
    platform: 'browser',
    format: 'esm',
    target: 'es2023',
    outfile: 'dist/jose.js',
    legalComments: 'inline',
    banner: { js: `/*! jose 6.2.18\n${license}\n*/` },
});
