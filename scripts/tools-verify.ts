#!/usr/bin/env node

// tools-verify: the pre-deploy checks for dev/tools HTML apps that deploy.ts can't
// catch (it minifies and PUTs; it never parses the JS):
//  1. every inline <script> parses (modules are checked as async function bodies;
//     JSON-LD and `text/plain` bundle placeholders are skipped);
//  2. the `@su:responder-script` block, if any, parses as the expression deploy.ts ships;
//  3. html-minifier-terser accepts the page with deploy.ts's exact options.
//
// Usage: `make tools-verify [ARGS="dev/tools/md.html ..."]` (default: every tool).
// Read-only, no browser, no network. Exits non-zero on any failure.

import { readdir, readFile } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import * as vm from "node:vm";
import { minify } from "html-minifier-terser";
import { MINIFY_OPTIONS } from "../dev/tools/deploy.ts";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const TOOLS_DIR = resolve(REPO_ROOT, "dev/tools");

const args = process.argv.slice(2);
const files = args.length
  ? args.map((f) => resolve(f))
  : (await readdir(TOOLS_DIR)).filter((f) => f.endsWith(".html") && f !== "og-template.html").map((f) => resolve(TOOLS_DIR, f));

let failed = 0;
for (const file of files) {
  const name = relative(REPO_ROOT, file);
  const html = await readFile(file, "utf8");
  const errors: string[] = [];

  let index = 0;
  for (const [, attrs, code] of html.matchAll(/<script((?:[^>])*)>([\s\S]*?)<\/script>/g)) {
    index++;
    if (!code.trim() || /\bsrc\s*=/.test(attrs) || /type\s*=\s*["'](application\/ld\+json|text\/plain)["']/i.test(attrs)) continue;
    const source = /type\s*=\s*["']module["']/i.test(attrs) ? `(async () => {\n${code}\n})` : code;
    try { new vm.Script(source, { filename: `${name}#script${index}` }); }
    catch (err) { errors.push(`script #${index}: ${(err as Error).message}`); }
  }

  const responder = html.match(/<!--\s*@su:responder-script\s*([\s\S]*?)-->/)?.[1];
  if (responder) {
    try { new vm.Script(`globalThis.__r = ${responder.trim().replace(/;+\s*$/, "")}`, { filename: `${name}#responder-script` }); }
    catch (err) { errors.push(`responder script: ${(err as Error).message}`); }
  }

  try { await minify(html, MINIFY_OPTIONS); }
  catch (err) { errors.push(`minifier: ${(err as Error).message}`); }

  if (errors.length) {
    failed++;
    console.log(`FAIL ${name}`);
    for (const e of errors) console.log(`     - ${e}`);
  } else {
    console.log(`OK   ${name}`);
  }
}
process.exit(failed ? 1 : 0);
