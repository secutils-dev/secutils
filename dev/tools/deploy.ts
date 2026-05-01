import { readFileSync, readdirSync } from "node:fs";
import { join, basename, dirname, resolve } from "node:path";
import { minify } from "html-minifier-terser";

const TOOLS_DIR = resolve(dirname(process.argv[1]));
const PREFIX = "deploy-tools";

const ANSI = {
  red: (s: string) => `\x1b[31m${s}\x1b[0m`,
  green: (s: string) => `\x1b[32m${s}\x1b[0m`,
  yellow: (s: string) => `\x1b[33m${s}\x1b[0m`,
  cyan: (s: string) => `\x1b[36m${s}\x1b[0m`,
  dim: (s: string) => `\x1b[2m${s}\x1b[0m`,
  bold: (s: string) => `\x1b[1m${s}\x1b[0m`,
};

function log(msg: string) {
  console.log(`${ANSI.bold(PREFIX)}: ${msg}`);
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  return kb < 1024 ? `${kb.toFixed(1)} KB` : `${(kb / 1024).toFixed(1)} MB`;
}

function filenameToEnvKey(filename: string): string {
  const stem = basename(filename, ".html");
  return `SECUTILS_HTML_APP_RESPONDER_ID_${stem.replace(/-/g, "_").toUpperCase()}`;
}

// Extracts the responder script embedded in an HTML comment of the form:
//   <!-- @su:responder-script
//   ...JS code...
//   -->
// The marker comment is stripped from the deployed body by html-minifier-terser
// (`removeComments: true`); we lift its payload here so it can be sent in the
// same PUT as the responder's `script` setting. See dev/tools/AGENTS.md for the
// full convention.
const RESPONDER_SCRIPT_RE = /<!--\s*@su:responder-script\s*\r?\n([\s\S]*?)\s*-->/g;

function extractResponderScript(
  rawHtml: string,
  label: string,
): string | undefined {
  const matches = [...rawHtml.matchAll(RESPONDER_SCRIPT_RE)];
  if (matches.length === 0) return undefined;
  if (matches.length > 1) {
    console.log(
      `  ${label}  ${ANSI.yellow(`⚠ multiple @su:responder-script comments found, using the first`)}`,
    );
  }
  return matches[0][1].trim();
}

async function main() {
  const API_DOMAIN = process.env.SECUTILS_HTML_APP_API_DOMAIN;
  const API_KEY = process.env.SECUTILS_HTML_APP_API_KEY;

  if (!API_DOMAIN) {
    log(ANSI.red("error: SECUTILS_HTML_APP_API_DOMAIN is not set in .env"));
    process.exit(1);
  }

  if (!API_KEY) {
    log(ANSI.red("error: SECUTILS_HTML_APP_API_KEY is not set in .env"));
    process.exit(1);
  }

  const allHtmlFiles = readdirSync(TOOLS_DIR)
    .filter((f) => f.endsWith(".html"))
    .sort();
  const allNames = allHtmlFiles.map((f) => basename(f, ".html"));

  const cliArgs = process.argv.slice(2);
  let targetFiles: string[];

  if (cliArgs.length === 0) {
    targetFiles = allHtmlFiles;
  } else {
    const unknown: string[] = [];
    targetFiles = [];
    for (const arg of cliArgs) {
      const name = arg.endsWith(".html") ? arg : `${arg}.html`;
      if (allHtmlFiles.includes(name)) {
        targetFiles.push(name);
      } else {
        unknown.push(arg);
      }
    }
    if (unknown.length > 0) {
      log(ANSI.red(`error: unknown tool(s): ${unknown.join(", ")}`));
      log(`available: ${allNames.join(", ")}`);
      process.exit(1);
    }
  }

  log(`API domain: ${ANSI.cyan(API_DOMAIN)}`);
  if (cliArgs.length > 0) {
    log(`deploying ${targetFiles.length} tool(s): ${targetFiles.join(", ")}`);
  } else {
    log(`deploying ${targetFiles.length} tool(s)...`);
  }
  console.log();

  const padLen = Math.max(...targetFiles.map((f) => f.length));
  let deployed = 0;
  let skipped = 0;
  let failed = 0;

  for (const file of targetFiles) {
    const label = file.padEnd(padLen);
    const envKey = filenameToEnvKey(file);
    const responderId = process.env[envKey];

    if (!responderId) {
      console.log(
        `  ${label}  ${ANSI.yellow(`⚠ skipped (no responder ID, expected ${envKey})`)}`,
      );
      skipped++;
      continue;
    }

    let originalSize: number;
    let rawHtml: string;
    try {
      rawHtml = readFileSync(join(TOOLS_DIR, file), "utf-8");
      originalSize = Buffer.byteLength(rawHtml, "utf-8");
    } catch (err) {
      console.log(`  ${label}  ${ANSI.red(`✗ read error: ${err}`)}`);
      failed++;
      continue;
    }

    const responderScript = extractResponderScript(rawHtml, label);

    let minified: string;
    try {
      minified = await minify(rawHtml, {
        collapseWhitespace: true,
        removeComments: true,
        minifyCSS: true,
        minifyJS: true,
        removeRedundantAttributes: true,
        removeScriptTypeAttributes: true,
        removeStyleLinkTypeAttributes: true,
      });
    } catch (err) {
      console.log(`  ${label}  ${ANSI.red(`✗ minify error: ${err}`)}`);
      failed++;
      continue;
    }

    const minifiedSize = Buffer.byteLength(minified, "utf-8");
    const savedPct = (((originalSize - minifiedSize) / originalSize) * 100).toFixed(1);
    const scriptInfo = responderScript
      ? ` ${ANSI.dim(`+ script ${formatSize(Buffer.byteLength(responderScript, "utf-8"))}`)}`
      : "";
    const sizeInfo = `${formatSize(originalSize)} -> ${formatSize(minifiedSize)} ${ANSI.dim(`(${savedPct}% saved)`)}${scriptInfo}`;

    const settings: { statusCode: number; body: string; script?: string } = {
      statusCode: 200,
      body: minified,
      ...(responderScript ? { script: responderScript } : {}),
    };

    try {
      const url = `${API_DOMAIN}/api/webhooks/responders/${responderId}`;
      const res = await fetch(url, {
        method: "PUT",
        headers: {
          Authorization: `Bearer ${API_KEY}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ settings }),
      });

      if (res.ok || res.status === 204) {
        console.log(`  ${label}  ${sizeInfo}  ${ANSI.green("✓ deployed")}`);
        deployed++;
      } else {
        const body = await res.text();
        console.log(
          `  ${label}  ${sizeInfo}  ${ANSI.red(`✗ HTTP ${res.status}: ${body.slice(0, 200)}`)}`,
        );
        failed++;
      }
    } catch (err) {
      console.log(`  ${label}  ${sizeInfo}  ${ANSI.red(`✗ upload error: ${err}`)}`);
      failed++;
    }
  }

  console.log();
  const parts = [`${deployed}/${targetFiles.length} deployed`];
  if (skipped > 0) parts.push(`${skipped} skipped`);
  if (failed > 0) parts.push(ANSI.red(`${failed} failed`));
  log(parts.join(", "));

  process.exit(failed > 0 ? 1 : 0);
}

main();
