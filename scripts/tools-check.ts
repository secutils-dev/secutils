#!/usr/bin/env node

// tools-check: verifies that the marketing site's hard-coded "Free single-page
// tools" list stays in sync with the source of truth - the
// `<meta name="su-tool-promote">` tag in every `dev/tools/*.html`.
//
// Concretely it asserts:
//  1. Every tool whose source HTML carries `su-tool-promote=true` appears at
//     least once in the marketing site's home page as an anchor pointing at
//     `https://{{TOOLS_HOST}}/<path>`. The marketing site lives in a separate
//     (private) sibling checkout; point at its `index.html` via the
//     `SECUTILS_TOOLS_PROMO_HOME_INDEX` env var (absolute path, or a path
//     relative to this repo root). When the env var is unset, this check is
//     skipped with a warning so contributors without the marketing checkout
//     can still run `make tools-check`.
//  2. Every promoted tool is listed in the README's "Free single-page tools"
//     table (`README.md`).
//  3. Every promoted tool has a corresponding `<slug>.skill.md` in
//     `dev/tools/`.
//  4. No non-promoted tool (e.g. mock-saml-idp) is referenced in the promo
//     home strip / cards section - those tools live on the index page only.
//
// Read-only, no browser, no Docker. Run with `make tools-check` (or
// `node scripts/tools-check.ts` - Node 24+ supports `.ts` natively via type
// stripping, no transpile step). Exits non-zero on any drift so it's safe to
// wire into CI.

import { readdir, readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = resolve(HERE, "..");
const TOOLS_DIR = resolve(REPO_ROOT, "dev/tools");
const PROMO_INDEX_ENV = "SECUTILS_TOOLS_PROMO_HOME_INDEX";
const PROMO_INDEX_RAW = process.env[PROMO_INDEX_ENV];
const PROMO_INDEX = PROMO_INDEX_RAW ? resolve(REPO_ROOT, PROMO_INDEX_RAW) : null;
const README = resolve(REPO_ROOT, "README.md");

type Tool = {
  file: string;
  path: string;
  name: string;
  promote: boolean;
};

const META_RE = (name: string): RegExp =>
  new RegExp(`<meta\\s+name=["']${name}["']\\s+content=["']([^"']*)["']`, "i");

async function readTools(): Promise<Tool[]> {
  const entries = await readdir(TOOLS_DIR);
  const tools: Tool[] = [];
  for (const file of entries) {
    if (!file.endsWith(".html") || file === "og-template.html") continue;
    const html = await readFile(resolve(TOOLS_DIR, file), "utf8");
    const path = META_RE("su-tool-path").exec(html)?.[1];
    const name = META_RE("su-tool-name").exec(html)?.[1];
    const promote = META_RE("su-tool-promote").exec(html)?.[1];
    if (!path || !name || promote == null) {
      throw new Error(
        `${file} is missing one of su-tool-path / su-tool-name / su-tool-promote - required by AGENTS.md`,
      );
    }
    tools.push({ file, path, name, promote: promote === "true" });
  }
  return tools;
}

async function main(): Promise<void> {
  const tools = await readTools();
  const promoted = tools.filter((t) => t.promote);
  const nonPromoted = tools.filter((t) => !t.promote);
  const errors: string[] = [];

  let promoHtml = "";
  if (!PROMO_INDEX) {
    console.warn(
      `tools-check: ${PROMO_INDEX_ENV} not set - skipping promo home page sync check. Set it to the absolute path of the marketing site's index.html (or a path relative to this repo root) to enable.`,
    );
  } else {
    try {
      promoHtml = await readFile(PROMO_INDEX, "utf8");
    } catch (err) {
      errors.push(
        `Could not read the promo home page at ${PROMO_INDEX} (configured via ${PROMO_INDEX_ENV}). (${(err as Error).message})`,
      );
    }
  }

  let readme = "";
  try {
    readme = await readFile(README, "utf8");
  } catch (err) {
    errors.push(`Could not read README.md: ${(err as Error).message}`);
  }

  // (1) every promoted tool appears at least once in the promo home page.
  if (promoHtml) {
    for (const t of promoted) {
      const needle = `https://{{TOOLS_HOST}}${t.path}`;
      if (!promoHtml.includes(needle)) {
        errors.push(
          `Promoted tool "${t.name}" (${t.path}) is NOT linked from the promo home page (looked for "${needle}" in ${PROMO_INDEX}).`,
        );
      }
    }

    // (4) no non-promoted tool is linked from the promo home strip. The
    // index page itself (path `/`) is always linked from the home page via
    // "See all" / "Browse all tools" anchors, so we skip it here.
    for (const t of nonPromoted) {
      if (t.path === "/") continue;
      const needle = `https://{{TOOLS_HOST}}${t.path}`;
      if (promoHtml.includes(needle)) {
        errors.push(
          `Non-promoted tool "${t.name}" (${t.path}) leaked into the promo home page; it should appear only on the index page. Either set su-tool-promote=true or remove the link in ${PROMO_INDEX}.`,
        );
      }
    }
  }

  // (2) every promoted tool is listed in the README table.
  if (readme) {
    for (const t of promoted) {
      const needle = `https://tools.secutils.dev${t.path}`;
      if (!readme.includes(needle)) {
        errors.push(
          `Promoted tool "${t.name}" (${t.path}) is NOT listed in README.md "Free single-page tools" subsection (expected an absolute "${needle}" link).`,
        );
      }
    }
  }

  // (3) every promoted tool has a sibling skill .md.
  const dirEntries = new Set(await readdir(TOOLS_DIR));
  for (const t of promoted) {
    const expected = t.file.replace(/\.html$/, ".skill.md");
    if (!dirEntries.has(expected)) {
      errors.push(
        `Promoted tool "${t.name}" (${t.file}) is missing the sibling AI-agent skill at ${expected} (required by AGENTS.md → "AI-agent surface").`,
      );
    }
  }

  if (errors.length > 0) {
    console.error("tools-check: FAIL");
    for (const e of errors) console.error("  -", e);
    process.exit(1);
  }

  console.log(
    `tools-check: OK (${tools.length} tools, ${promoted.length} promoted, ${nonPromoted.length} index-only)`,
  );
}

main().catch((err: unknown) => {
  console.error("tools-check: crashed:", err);
  process.exit(2);
});
