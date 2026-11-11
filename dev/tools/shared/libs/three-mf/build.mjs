import { build } from 'esbuild';
import { readFileSync } from 'node:fs';

// saxes omits its license file from the npm tarball. Keep the upstream notice locally.
const licenses = ['node_modules/fflate/LICENSE', 'LICENSE.saxes', 'node_modules/xmlchars/LICENSE'];
const notices = licenses.map((path) => readFileSync(path, 'utf8')).join('\n\n');
const worker = await build({ entryPoints: ['src/worker.js'], bundle: true, minify: true, write: false, platform: 'browser', format: 'iife', legalComments: 'inline', banner: { js: `/*! ${notices.replaceAll('*/', '* /')} */` } });
await build({ stdin: { contents: `export const workerSource = ${JSON.stringify(worker.outputFiles[0].text)};`, resolveDir: process.cwd() }, bundle: true, minify: true, outfile: 'dist/three-mf.js', format: 'esm' });
