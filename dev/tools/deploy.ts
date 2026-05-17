import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { gzipSync } from "node:zlib";
import { minify } from "html-minifier-terser";

const TOOLS_DIR = resolve(dirname(process.argv[1]));
// Each sub-directory under `dev/tools/js/` is an isolated build of one JS
// bundle (own package.json + lockfile + Vite/Rollup config). The deploy
// pipeline inlines these into HTML responders that reference them via a
// `data-su-bundle="<name>"` placeholder. See dev/tools/AGENTS.md ->
// "Embedded JS bundles (`data-su-bundle`)".
const BUNDLES_DIR = resolve(TOOLS_DIR, "js");
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

// `jwt-debugger.html`     -> `JWT_DEBUGGER`
// `jwt-debugger.skill.md` -> `JWT_DEBUGGER_MD`
// `index.html`            -> `INDEX`
// `llms.txt`              -> `LLMS_TXT`
function filenameToEnvKey(filename: string): string {
  let stem = filename;
  if (stem.endsWith(".skill.md")) stem = stem.slice(0, -".skill.md".length) + ".md";
  if (stem.endsWith(".html")) stem = stem.slice(0, -".html".length);
  return `SECUTILS_HTML_APP_RESPONDER_ID_${stem.replace(/[-.]/g, "_").toUpperCase()}`;
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

// Substitute `{{TOOLS_HOST}}` placeholders with the configured public host.
// Used in HTML <head> (canonical, og:url, og:image, JSON-LD url, related-tools
// hrefs) and in skill .md frontmatter. One source-of-truth knob, one re-deploy
// to change the public host of every tool.
function substituteToolsHost(text: string, toolsHost: string): string {
  return text.replace(/\{\{\s*TOOLS_HOST\s*\}\}/g, toolsHost);
}

// -----------------------------------------------------------------------------
// Generic JS bundle inliner (data-su-bundle)
// -----------------------------------------------------------------------------
//
// Author-side convention (in any tool HTML):
//
//   <script id="su-bundle-liteparse" type="text/plain"
//           data-su-bundle="liteparse"></script>
//
// The placeholder is empty (`type="text/plain"` keeps the browser from
// executing whatever lands inside on page load). At deploy time we discover
// every `data-su-bundle="<name>"` reference, ensure
// `dev/tools/js/<name>/dist/<name>.js` is fresh, and inject the bundle source
// as the placeholder's text content. The tool's own JS then lazy-imports it
// via a tiny helper:
//
//   const blob = new Blob([el.textContent], { type: 'text/javascript' });
//   const mod  = await import(URL.createObjectURL(blob));
//
// Build rule per sub-package:
//   dev/tools/js/<name>/package.json#scripts.build  ->  dist/<name>.js
//
// Idempotency: if `dist/<name>.js` exists and its mtime is >= the newest
// source mtime (everything under the sub-package except dist/ and
// node_modules/), the build is skipped. The deploy never silently ships a
// stale bundle.

// Matches `<script ... data-su-bundle="<name>" ...></script>` placeholders.
// Captures the opening tag (group 1) and the bundle name (group 2). The body
// must be empty (whitespace only) -- the inliner refuses to overwrite a tag
// that already has content, both as a safety net and to keep the convention
// honest. The tag survives html-minifier-terser unchanged because the body
// is empty and `data-*` attributes are never stripped.
const BUNDLE_PLACEHOLDER_RE =
  /(<script\b[^>]*\bdata-su-bundle\s*=\s*["']([a-z0-9_-]+)["'][^>]*>)\s*<\/script>/gi;

// Bundle names that may appear in any HTML. We accept lowercase, digits,
// dashes, and underscores; the same charset that's allowed in URL slugs so
// `data-su-bundle` is easy to grep across the repo.
const BUNDLE_NAME_RE = /^[a-z0-9_-]+$/;

function discoverBundleNames(html: string): string[] {
  const names = new Set<string>();
  for (const m of html.matchAll(BUNDLE_PLACEHOLDER_RE)) {
    names.add(m[2]);
  }
  return [...names];
}

// Walks `root` recursively, returning the largest mtime (ms) of any file,
// excluding any directory whose basename matches `excludeDirs`. Used to
// decide whether `dist/<name>.js` is older than its sources.
function newestMtimeUnder(root: string, excludeDirs: Set<string>): number {
  let newest = 0;
  const stack: string[] = [root];
  while (stack.length > 0) {
    const dir = stack.pop()!;
    let entries: string[];
    try {
      entries = readdirSync(dir);
    } catch {
      continue;
    }
    for (const entry of entries) {
      const full = join(dir, entry);
      let st;
      try {
        st = statSync(full);
      } catch {
        continue;
      }
      if (st.isDirectory()) {
        if (excludeDirs.has(entry)) continue;
        stack.push(full);
      } else if (st.mtimeMs > newest) {
        newest = st.mtimeMs;
      }
    }
  }
  return newest;
}

// Caches built bundle source so repeated calls (across HTML files that share
// a bundle) don't re-stat / re-read it. Keyed by bundle name.
const BUNDLE_CACHE = new Map<string, string>();

// Ensures `dev/tools/js/<name>/dist/<name>.js` exists and is up to date
// relative to the sub-package's sources, building it on demand. Returns the
// bundle source as a string. Throws if the sub-package directory or its
// `package.json` is missing, or the build fails.
function ensureBundleBuilt(name: string): string {
  if (!BUNDLE_NAME_RE.test(name)) {
    throw new Error(`invalid data-su-bundle name "${name}"`);
  }
  const cached = BUNDLE_CACHE.get(name);
  if (cached !== undefined) return cached;

  const bundleDir = resolve(BUNDLES_DIR, name);
  if (!existsSync(bundleDir)) {
    throw new Error(
      `bundle "${name}" not found: expected sub-package at ${bundleDir}`,
    );
  }
  const pkgJson = resolve(bundleDir, "package.json");
  if (!existsSync(pkgJson)) {
    throw new Error(
      `bundle "${name}" is missing package.json at ${pkgJson}`,
    );
  }
  const distPath = resolve(bundleDir, "dist", `${name}.js`);
  const exclude = new Set(["dist", "node_modules"]);
  const sourceMtime = newestMtimeUnder(bundleDir, exclude);
  let needsBuild = !existsSync(distPath);
  if (!needsBuild) {
    const distMtime = statSync(distPath).mtimeMs;
    if (sourceMtime > distMtime) needsBuild = true;
  }
  if (needsBuild) {
    log(`bundle ${ANSI.cyan(name)}: ${ANSI.dim("building...")}`);
    // `npm ci` is the slow part; only run it when node_modules is absent.
    // Subsequent rebuilds (e.g. a stub tweak) skip straight to `npm run build`.
    if (!existsSync(resolve(bundleDir, "node_modules"))) {
      execFileSync("npm", ["ci"], { cwd: bundleDir, stdio: "inherit" });
    }
    execFileSync("npm", ["run", "build"], { cwd: bundleDir, stdio: "inherit" });
    if (!existsSync(distPath)) {
      throw new Error(
        `bundle "${name}" build finished but ${distPath} was not produced. ` +
          `Check the sub-package's vite/rollup output config.`,
      );
    }
  }
  const code = readFileSync(distPath, "utf-8");
  BUNDLE_CACHE.set(name, code);
  return code;
}

// Detects `data-su-bundle-encoding="gzip-base64"` on the placeholder's
// opening tag. Bundles flagged with this encoding are gzipped + base64'd
// before inlining; the tool's runtime loader is expected to reverse the two
// steps (DecompressionStream + atob) before Blob-URL'ing the result. The
// encoding exists because some responder backends cap the PUT JSON payload
// (~2 MB today); a 3 MB raw bundle balloons to ~3.3 MB JSON-encoded and is
// rejected, while gzip+base64 typically lands at ~1 MB JSON-encoded for the
// same bundle. Tradeoff: ~10-20 ms of one-time decompression on first use.
const BUNDLE_ENCODING_RE =
  /\bdata-su-bundle-encoding\s*=\s*["']([a-z0-9_-]+)["']/i;

// Replaces every `data-su-bundle` placeholder in `html` with the corresponding
// bundle source. Run *after* html-minifier-terser so the (potentially several
// MB of) bundle source never passes through the minifier (it's already
// minified by Vite/Rollup and the minifier has no useful work to do on it,
// while a `collapseWhitespace` quirk could theoretically corrupt it). The
// `</script>` substring inside the bundle is escaped to `<\/script>` so the
// inlined `<script>` tag doesn't terminate early -- standard same-origin
// HTML embedding hygiene.
function inlineBundles(html: string): {
  html: string;
  bundleBytes: Map<string, number>;
} {
  const bundleBytes = new Map<string, number>();
  const out = html.replace(
    BUNDLE_PLACEHOLDER_RE,
    (_match, openTag: string, name: string) => {
      const code = ensureBundleBuilt(name);
      const encoding = BUNDLE_ENCODING_RE.exec(openTag)?.[1];
      let payload: string;
      if (!encoding) {
        // Raw text inlining: escape `</script>` so the host <script> tag
        // can't terminate early.
        payload = code.replace(/<\/script>/gi, "<\\/script>");
      } else if (encoding === "gzip-base64") {
        // Gzip the UTF-8 bundle source, then base64-encode. The base64
        // alphabet is `</script>`-safe, so no further escaping is needed.
        const gz = gzipSync(Buffer.from(code, "utf-8"), { level: 9 });
        payload = gz.toString("base64");
      } else {
        throw new Error(
          `bundle "${name}": unsupported data-su-bundle-encoding="${encoding}" ` +
            `(known: <unset> for raw text, "gzip-base64" for compressed)`,
        );
      }
      bundleBytes.set(
        name,
        (bundleBytes.get(name) ?? 0) + Buffer.byteLength(payload, "utf-8"),
      );
      return `${openTag}${payload}</script>`;
    },
  );
  return { html: out, bundleBytes };
}

// llms.txt entry, sourced from the corresponding tool HTML's `su-tool-*`
// meta tags rather than from skill .md frontmatter. The skill files
// themselves are real Claude Code / Cursor SKILL.md documents (terse `name`
// + `description` frontmatter, rich Markdown body) and intentionally do not
// carry registry metadata. The HTML is the canonical source for path / name
// / description / promotion because it's also what `tools-check.mjs`,
// `e2e/tools/registry.ts`, and the marketing site key off.
type ToolMeta = {
  // Slug of the source file (`.html` stripped, dashes preserved).
  slug: string;
  // su-tool-name content (human-readable name).
  name: string;
  // su-tool-path content (URL path under TOOLS_HOST).
  path: string;
  // su-tool-description content (one-line marketing description).
  description: string;
  // su-tool-promote content === "true".
  promote: boolean;
};

const META_RE = (name: string): RegExp =>
  new RegExp(`<meta\\s+name=["']${name}["']\\s+content=["']([^"']*)["']`, "i");

function parseToolMeta(rawHtml: string, slug: string): ToolMeta | null {
  const name = META_RE("su-tool-name").exec(rawHtml)?.[1];
  const path = META_RE("su-tool-path").exec(rawHtml)?.[1];
  const description = META_RE("su-tool-description").exec(rawHtml)?.[1];
  const promote = META_RE("su-tool-promote").exec(rawHtml)?.[1];
  if (!name || !path || !description || promote == null) return null;
  return { slug, name, path, description, promote: promote === "true" };
}

// Build the aggregate llms.txt body (https://llmstxt.org/ convention) from
// the tool HTML registry. Only promoted tools (`su-tool-promote=true`) are
// listed; non-promoted tools are reachable only via direct link and must not
// appear in any discovery surface (see `dev/tools/AGENTS.md` -> "Promotion").
// Each entry links to the `.md` SKILL companion because that is the canonical
// machine-readable form.
function buildLlmsTxt(tools: ToolMeta[], toolsHost: string): string {
  const ordered = tools.filter((t) => t.promote && t.path !== "/");
  const lines: string[] = [];
  lines.push("# Secutils.dev Tools");
  lines.push("");
  lines.push(
    "> Free, no-signup, single-page developer and security tools. Each tool",
  );
  lines.push(
    "> publishes a SKILL.md companion at <url>.md (Claude Code / Cursor",
  );
  lines.push(
    "> compatible) describing how to drive it from an AI agent, including",
  );
  lines.push("> the URL-state wire format where applicable.");
  lines.push("");
  lines.push("## Tools");
  lines.push("");
  for (const t of ordered) {
    lines.push(
      `- [${t.name}](https://${toolsHost}${t.path}.md): ${t.description}`,
    );
  }
  lines.push("");
  lines.push("## Wire format");
  lines.push("");
  lines.push(
    "Most tools deep-link via the URL fragment using a single shared encoding",
  );
  lines.push(
    "(deflate-raw + 4-byte LE u32 ulen prefix + base64url, of UTF-8 string or",
  );
  lines.push(
    "JSON.stringify(state)). Each SKILL.md includes a runnable Node snippet to",
  );
  lines.push("produce the URL. Reference and full spec:");
  lines.push(
    "https://github.com/secutils-dev/secutils/blob/main/dev/tools/AGENTS.md#url-state-encoding-encodestate--decodestate",
  );
  lines.push("");
  lines.push(`Index: https://${toolsHost}/`);
  lines.push("");
  return lines.join("\n");
}

// Generated agent-discovery surfaces (https://isitagentready.com guidance).

// `/robots.txt` -- explicit allow-list for AI crawlers, Content-Signal
// directives declaring our preferences (we *want* to be agent-indexed and
// agent-driven, since these are free public tools), and a Sitemap reference.
function buildRobotsTxt(toolsHost: string): string {
  // AI crawlers we explicitly allow. Order copied from
  // https://developers.cloudflare.com/ai-crawl-control/. Adding more is a
  // no-op; the wildcard `User-agent: *` rule below also allows them.
  const aiAgents = [
    "GPTBot",
    "OAI-SearchBot",
    "ChatGPT-User",
    "ClaudeBot",
    "Claude-Web",
    "anthropic-ai",
    "Google-Extended",
    "PerplexityBot",
    "Perplexity-User",
    "Applebot-Extended",
    "cohere-ai",
    "CCBot",
    "Bytespider",
    "Diffbot",
    "DuckAssistBot",
    "Meta-ExternalAgent",
    "Amazonbot",
    "FacebookBot",
  ];
  const lines: string[] = [];
  lines.push("# Free, no-signup developer and security tools.");
  lines.push("# Everything here is public and intended to be indexed by humans,");
  lines.push("# search engines, and AI agents. There is nothing private to crawl.");
  lines.push("");
  lines.push("User-agent: *");
  lines.push("Allow: /");
  lines.push("");
  for (const ua of aiAgents) {
    lines.push(`User-agent: ${ua}`);
    lines.push("Allow: /");
    lines.push("");
  }
  // https://contentsignals.org/ - declares that AI training, search indexing,
  // and AI input (RAG / agent retrieval) are all welcome on these tools.
  lines.push("# Content Signals (https://contentsignals.org/)");
  lines.push("Content-Signal: ai-train=yes, search=yes, ai-input=yes");
  lines.push("");
  lines.push(`Sitemap: https://${toolsHost}/sitemap.xml`);
  lines.push("");
  return lines.join("\n");
}

// `/sitemap.xml` -- one entry per public surface so search engines and agent
// crawlers (e.g. agent-skills indexers) can fan out from a single document.
// Only the index page and promoted tools are listed; non-promoted tools are
// reachable only via direct link and intentionally excluded from the sitemap
// (and carry their own `<meta name="robots" content="noindex, nofollow">`).
function buildSitemapXml(tools: ToolMeta[], toolsHost: string): string {
  const ordered = [
    ...tools.filter((t) => t.path === "/"),
    ...tools.filter((t) => t.promote && t.path !== "/"),
  ];
  const today = new Date().toISOString().slice(0, 10);
  const urls: { loc: string; priority: string; changefreq: string }[] = [];
  // Index page first.
  urls.push({ loc: `https://${toolsHost}/`, priority: "1.0", changefreq: "weekly" });
  // Each tool's HTML and its `.md` SKILL companion. The `.md` shares the
  // same priority -- agent crawlers value it as much as the HTML.
  for (const t of ordered) {
    if (t.path === "/") continue;
    urls.push({ loc: `https://${toolsHost}${t.path}`, priority: "0.9", changefreq: "weekly" });
    urls.push({ loc: `https://${toolsHost}${t.path}.md`, priority: "0.9", changefreq: "weekly" });
  }
  // Aggregate / discovery surfaces.
  urls.push({ loc: `https://${toolsHost}/llms.txt`, priority: "0.7", changefreq: "weekly" });
  urls.push({
    loc: `https://${toolsHost}/.well-known/agent-skills/index.json`,
    priority: "0.7",
    changefreq: "weekly",
  });

  const lines: string[] = [];
  lines.push('<?xml version="1.0" encoding="UTF-8"?>');
  lines.push('<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">');
  for (const u of urls) {
    lines.push("  <url>");
    lines.push(`    <loc>${u.loc}</loc>`);
    lines.push(`    <lastmod>${today}</lastmod>`);
    lines.push(`    <changefreq>${u.changefreq}</changefreq>`);
    lines.push(`    <priority>${u.priority}</priority>`);
    lines.push("  </url>");
  }
  lines.push("</urlset>");
  lines.push("");
  return lines.join("\n");
}

// `/.well-known/agent-skills/index.json` -- Cloudflare's Agent Skills
// Discovery RFC v0.2.0 format (https://github.com/cloudflare/agent-skills-discovery-rfc).
// One entry per deployed `<slug>.skill.md`. The digest of each skill body is
// included (as `sha256:<hex>` per the spec) so agent skill loaders can detect
// updates without re-fetching.
//
// Strict v0.2.0 conformance (was wrong in earlier deploys, fixed for review
// feedback from Cloudflare):
//   - `$schema` is the canonical `https://schemas.agentskills.io/...` URL,
//     not the `agentskills.io/schema/...` variant.
//   - `type` is `"skill-md"` (was `"skill"`).
//   - Integrity field is `digest: "sha256:<hex>"` (was a bare `sha256: <hex>`).
//   - `name` is taken from the SKILL.md YAML frontmatter `name:` field, NOT
//     from the file slug. The slug is a deploy-time path concern; the skill's
//     canonical identifier (e.g. `pem-certificate-decoder`, `mock-response`)
//     lives in the SKILL.md itself, where it must match the Agent Skills
//     spec naming rules and stay in sync with the promo site's
//     `/.well-known/agent-skills/index.json`, which keys off the same field.
type SkillIndexEntry = {
  name: string;
  type: "skill-md";
  description: string;
  url: string;
  digest: string;
};

// Extracts the `name:` value from a SKILL.md YAML frontmatter block. The
// frontmatter is always the first `---`-delimited block at the top of the
// file (we generate it that way ourselves). Returns `undefined` if there is
// no frontmatter or no `name:` line -- the caller treats that as a hard
// error rather than silently falling back to the slug, because a wrong name
// in the discovery index makes the skill indistinguishable from a different
// one cached by clients keying on `name`.
const FRONTMATTER_RE = /^---\r?\n([\s\S]*?)\r?\n---/;
const FRONTMATTER_NAME_RE = /^name:\s*["']?([^"'\r\n]+?)["']?\s*$/m;

function extractSkillName(body: string): string | undefined {
  const block = FRONTMATTER_RE.exec(body);
  if (!block) return undefined;
  const name = FRONTMATTER_NAME_RE.exec(block[1])?.[1]?.trim();
  return name || undefined;
}

function buildAgentSkillsIndex(
  tools: ToolMeta[],
  skillBodies: Map<string, string>,
  toolsHost: string,
): string {
  // Only promoted tools advertise their skill in the discovery index;
  // non-promoted skills are still served at `<path>.md` for direct fetching.
  const ordered = tools.filter((t) => t.promote && t.path !== "/");
  const skills: SkillIndexEntry[] = [];
  const seenNames = new Set<string>();
  for (const t of ordered) {
    const body = skillBodies.get(t.slug);
    if (!body) continue;
    const name = extractSkillName(body);
    if (!name) {
      throw new Error(
        `agent-skills index: ${t.slug}.skill.md is missing a \`name:\` field in its YAML frontmatter`,
      );
    }
    if (seenNames.has(name)) {
      throw new Error(
        `agent-skills index: duplicate skill name "${name}" -- two SKILL.md files share the same frontmatter \`name:\``,
      );
    }
    seenNames.add(name);
    const hex = createHash("sha256").update(body, "utf-8").digest("hex");
    skills.push({
      name,
      type: "skill-md",
      description: t.description,
      url: `https://${toolsHost}${t.path}.md`,
      digest: `sha256:${hex}`,
    });
  }
  const doc = {
    // Canonical RFC v0.2.0 schema URL. See
    // https://github.com/cloudflare/agent-skills-discovery-rfc for the spec.
    $schema: "https://schemas.agentskills.io/discovery/0.2.0/schema.json",
    skills,
  };
  return JSON.stringify(doc, null, 2) + "\n";
}

// Wraps a tool's `@su:responder-script` body (or no script at all) with a
// Markdown content-negotiation prelude so requests with `Accept: text/markdown`
// get a 302 redirect to the `<slug>.md` SKILL companion. Browsers and
// `curl --compressed` (which sends `Accept: */*`) see no behaviour change.
//
// The prelude is intentionally tiny (~250 B) and pure: no sniffing of `User-Agent`,
// no quality-value parsing -- if the Accept header literally contains
// `text/markdown` and does NOT start with `text/html`, we serve the redirect.
// That covers every realistic agent / WebFetch UA we've seen and never trips a
// browser. See `dev/tools/AGENTS.md` -> "Markdown content negotiation".
const MD_NEGOTIATION_PRELUDE = `
  // SU: Markdown content negotiation prelude (auto-injected by deploy.ts).
  {
    const a = (context.headers && context.headers['accept'] || '').toLowerCase();
    if (a.includes('text/markdown') && !a.startsWith('text/html')) {
      return {
        statusCode: 302,
        headers: { Location: '__SU_MD_PATH__', Vary: 'Accept', 'Content-Type': 'text/plain; charset=utf-8' },
        body: '',
      };
    }
  }
`;

function wrapWithMdNegotiation(
  userScript: string | undefined,
  mdPath: string,
): string {
  const prelude = MD_NEGOTIATION_PRELUDE.replace(
    "__SU_MD_PATH__",
    // mdPath is one of our own files (e.g. "/echo.md", "/llms.txt"); no quoting
    // hazard, but be defensive against accidental backslashes / quotes anyway.
    mdPath.replace(/\\/g, "\\\\").replace(/'/g, "\\'"),
  );
  // Compose: outer IIFE runs the prelude first (which may `return` a 302),
  // then evaluates the user's existing IIFE expression and returns its value.
  // If there is no user script the wrapper returns null (= fall through to
  // the static body), preserving today's behaviour for script-less tools.
  const userExpr = (userScript ?? "null")
    .trim()
    .replace(/;+\s*$/, "");
  return `(() => {${prelude}  return (${userExpr});\n})()`;
}

type DeployTarget = {
  filename: string;
  body: string;
  contentType:
    | "text/html"
    | "text/markdown"
    | "text/plain"
    | "application/xml"
    | "application/json";
  // Only HTML responders carry a `script`; static text responders are body-only.
  script?: string;
  // Extra response headers (Link headers on the index page, ...). These are
  // merged with the implicit Content-Type header.
  extraHeaders?: [string, string][];
  // Reported as `original -> minified` in the deploy log. For Markdown / text
  // we don't minify so original == minified.
  originalSize: number;
  // Inlined `data-su-bundle` payload sizes, keyed by bundle name. Reported
  // alongside the body size so a 2 MB liteparse blob is visible in the deploy
  // log instead of mysteriously inflating the "minified" number.
  bundleBytes?: Map<string, number>;
};

async function buildHtmlTarget(
  filename: string,
  rawHtml: string,
  toolsHost: string,
  label: string,
  // The .md sibling path (e.g. "/echo.md", or "/llms.txt" for the index).
  // When set, the deployed responder script gets a Markdown-negotiation
  // prelude that 302s `Accept: text/markdown` requests there. When absent
  // (no skill deployed yet for this tool) the original script is used as-is
  // and the responder always serves HTML.
  mdNegotiationPath: string | undefined,
  // Static response headers (e.g. RFC 8288 Link headers on the index page).
  extraHeaders?: [string, string][],
): Promise<DeployTarget> {
  const templated = substituteToolsHost(rawHtml, toolsHost);
  const responderScript = extractResponderScript(templated, label);
  // `removeScriptTypeAttributes` would strip `type="text/javascript"` but
  // intentionally leaves non-default types like `text/plain` (which we use
  // for `data-su-bundle` placeholders) intact, so the convention survives.
  const minified = await minify(templated, {
    collapseWhitespace: true,
    removeComments: true,
    minifyCSS: true,
    minifyJS: true,
    removeRedundantAttributes: true,
    removeScriptTypeAttributes: true,
    removeStyleLinkTypeAttributes: true,
  });
  // Inline `data-su-bundle` payloads *after* minification: the bundle source
  // is already minified by Vite, and bypassing the HTML minifier avoids any
  // chance of `collapseWhitespace` / `minifyJS` corrupting the inlined ESM
  // module. See "Generic JS bundle inliner" above.
  const { html: withBundles, bundleBytes } = inlineBundles(minified);
  const finalScript = mdNegotiationPath
    ? wrapWithMdNegotiation(responderScript, mdNegotiationPath)
    : responderScript;
  return {
    filename,
    body: withBundles,
    contentType: "text/html",
    script: finalScript,
    extraHeaders,
    originalSize: Buffer.byteLength(templated, "utf-8"),
    bundleBytes: bundleBytes.size > 0 ? bundleBytes : undefined,
  };
}

function buildSkillTarget(
  filename: string,
  rawMd: string,
  toolsHost: string,
): DeployTarget {
  const body = substituteToolsHost(rawMd, toolsHost);
  return {
    filename,
    body,
    contentType: "text/markdown",
    originalSize: Buffer.byteLength(body, "utf-8"),
  };
}

function buildLlmsTxtTarget(body: string): DeployTarget {
  return {
    filename: "llms.txt",
    body,
    // The llms.txt body is real Markdown (headings, bullet lists, links), and
    // it is also the destination of the index page's `Accept: text/markdown`
    // 302 redirect. Serving it as `text/markdown` (a) is more accurate per
    // the llmstxt.org spec, and (b) makes the homepage pass Cloudflare's
    // markdown-for-agents content-negotiation contract -- the final response
    // in the redirect chain has the expected `Content-Type: text/markdown`.
    contentType: "text/markdown",
    originalSize: Buffer.byteLength(body, "utf-8"),
  };
}

function buildRobotsTxtTarget(body: string): DeployTarget {
  return {
    filename: "robots.txt",
    body,
    contentType: "text/plain",
    originalSize: Buffer.byteLength(body, "utf-8"),
  };
}

function buildSitemapXmlTarget(body: string): DeployTarget {
  return {
    filename: "sitemap.xml",
    body,
    contentType: "application/xml",
    originalSize: Buffer.byteLength(body, "utf-8"),
  };
}

function buildAgentSkillsIndexTarget(body: string): DeployTarget {
  return {
    filename: "agent-skills/index.json",
    body,
    contentType: "application/json",
    originalSize: Buffer.byteLength(body, "utf-8"),
  };
}

async function putResponder(
  apiDomain: string,
  apiKey: string,
  responderId: string,
  target: DeployTarget,
): Promise<{ ok: true } | { ok: false; status: number; body: string }> {
  // The Secutils responder accepts statusCode + body in `settings`. We set the
  // Content-Type via a `headers` array of [name, value] tuples (per the
  // ResponderSettings OpenAPI schema, `headers` is `array of array of string`,
  // not a map -- duplicates allowed, which is how multiple Link headers are
  // sent). For HTML the Content-Type is implied by the responder default; for
  // every other MIME we pin it explicitly so crawlers see the right type.
  const settings: {
    statusCode: number;
    body: string;
    headers?: [string, string][];
    script?: string;
  } = {
    statusCode: 200,
    body: target.body,
  };
  const headers: [string, string][] = [];
  switch (target.contentType) {
    case "text/markdown":
      headers.push(["Content-Type", "text/markdown; charset=utf-8"]);
      break;
    case "text/plain":
      headers.push(["Content-Type", "text/plain; charset=utf-8"]);
      break;
    case "application/xml":
      headers.push(["Content-Type", "application/xml; charset=utf-8"]);
      break;
    case "application/json":
      headers.push(["Content-Type", "application/json; charset=utf-8"]);
      break;
    case "text/html":
      // Default; do not pin so the responder can keep its existing behaviour.
      break;
  }
  if (target.extraHeaders) headers.push(...target.extraHeaders);
  if (headers.length > 0) settings.headers = headers;
  if (target.script) settings.script = target.script;

  const url = `${apiDomain}/api/webhooks/responders/${responderId}`;
  let res: Response;
  try {
    res = await fetch(url, {
      method: "PUT",
      headers: {
        Authorization: `Bearer ${apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ settings }),
    });
  } catch (err) {
    return { ok: false, status: 0, body: `fetch failed: ${(err as Error).message}` };
  }
  if (res.ok || res.status === 204) return { ok: true };
  const body = await res.text();
  return { ok: false, status: res.status, body };
}

async function main() {
  const API_DOMAIN = process.env.SECUTILS_HTML_APP_API_DOMAIN;
  const API_KEY = process.env.SECUTILS_HTML_APP_API_KEY;
  const TOOLS_HOST =
    process.env.SECUTILS_TOOLS_PUBLIC_HOST?.trim() || "tools.secutils.dev";

  if (!API_DOMAIN) {
    log(ANSI.red("error: SECUTILS_HTML_APP_API_DOMAIN is not set in .env"));
    process.exit(1);
  }

  if (!API_KEY) {
    log(ANSI.red("error: SECUTILS_HTML_APP_API_KEY is not set in .env"));
    process.exit(1);
  }

  // Discover sources. HTML files become the tool body; *.skill.md become the
  // companion AI skill. We deploy the HTML alongside its skill so deploys are
  // atomic per tool.
  const all = readdirSync(TOOLS_DIR).sort();
  const htmlFiles = all.filter((f) => f.endsWith(".html"));
  const skillFiles = all.filter((f) => f.endsWith(".skill.md"));
  const allNames = [
    ...htmlFiles.map((f) => basename(f, ".html")),
    ...skillFiles.map((f) => f.replace(/\.skill\.md$/, "") + ".skill.md"),
  ];

  // Allow filtering: `node deploy.ts jwt-debugger echo` deploys only those.
  // A bare slug matches both `<slug>.html` and `<slug>.skill.md`.
  const cliArgs = process.argv.slice(2);
  let targetFiles: string[];
  if (cliArgs.length === 0) {
    targetFiles = [...htmlFiles, ...skillFiles];
  } else {
    const expanded: string[] = [];
    const unknown: string[] = [];
    for (const arg of cliArgs) {
      const matches: string[] = [];
      const stem = arg.replace(/\.(html|skill\.md)$/, "");
      const html = `${stem}.html`;
      const skill = `${stem}.skill.md`;
      if (htmlFiles.includes(html)) matches.push(html);
      if (skillFiles.includes(skill)) matches.push(skill);
      // Allow exact filenames too.
      if (matches.length === 0 && htmlFiles.includes(arg)) matches.push(arg);
      if (matches.length === 0 && skillFiles.includes(arg)) matches.push(arg);
      if (matches.length === 0) unknown.push(arg);
      else expanded.push(...matches);
    }
    if (unknown.length > 0) {
      log(ANSI.red(`error: unknown tool(s): ${unknown.join(", ")}`));
      log(`available: ${allNames.join(", ")}`);
      process.exit(1);
    }
    targetFiles = expanded;
  }

  log(`API domain: ${ANSI.cyan(API_DOMAIN)}`);
  log(`tools host: ${ANSI.cyan(TOOLS_HOST)}`);
  if (cliArgs.length > 0) {
    log(`deploying ${targetFiles.length} responder(s): ${targetFiles.join(", ")}`);
  } else {
    log(`deploying ${targetFiles.length} responder(s) (all tools + skills + llms.txt)...`);
  }
  console.log();

  // Build the registry for llms.txt from the corresponding .html files'
  // `<meta name="su-tool-*">` tags (the canonical metadata source -- shared
  // with tools-check.mjs and the marketing site). A tool is listed iff:
  //   1. it has a sibling `<slug>.skill.md` on disk, AND
  //   2. that skill's `_MD` responder ID is configured in .env.
  // This keeps llms.txt honest during an incremental rollout: no 404 URLs
  // for AI crawlers, and the index grows automatically as the user adds
  // responder IDs. We also remember each templated skill body so the
  // agent-skills/index.json sha256 digests match the bytes actually served.
  const toolMetas: ToolMeta[] = [];
  const skillBodies = new Map<string, string>();
  for (const skillFile of skillFiles) {
    if (!process.env[filenameToEnvKey(skillFile)]) continue;
    const slug = skillFile.replace(/\.skill\.md$/, "");
    const htmlFile = `${slug}.html`;
    if (!htmlFiles.includes(htmlFile)) continue;
    const rawHtml = readFileSync(join(TOOLS_DIR, htmlFile), "utf-8");
    const meta = parseToolMeta(rawHtml, slug);
    if (!meta) continue;
    toolMetas.push(meta);
    const rawSkill = readFileSync(join(TOOLS_DIR, skillFile), "utf-8");
    skillBodies.set(slug, substituteToolsHost(rawSkill, TOOLS_HOST));
  }

  // For each HTML file, pre-compute the `.md` sibling path used by the
  // markdown content-negotiation prelude. We only set it when the .md is
  // actually deployable so a 302 from an Accept-negotiated request never
  // lands on a 404. Per-tool pages negotiate to `<path>.md`; the index page
  // negotiates to `/llms.txt` (its "agent" view), gated on _LLMS_TXT being
  // configured rather than on having an `index.skill.md` (there is none).
  const mdNegotiationPaths = new Map<string, string>();
  for (const t of toolMetas) {
    if (t.path !== "/") mdNegotiationPaths.set(t.slug, `${t.path}.md`);
  }
  if (
    htmlFiles.includes("index.html") &&
    process.env.SECUTILS_HTML_APP_RESPONDER_ID_LLMS_TXT
  ) {
    mdNegotiationPaths.set("index", "/llms.txt");
  }

  // Pin the RFC 8288 Link headers on the index responder so any agent that
  // fetches just `/` gets pointers to the discovery surfaces in response
  // headers (per https://isitagentready.com guidance). RFC 8288 §3 allows
  // (and recommends, for ordering predictability) combining multiple
  // link-values into a single `Link` header separated by commas. We do that
  // here because the responder's HeaderMap-style serializer collapses
  // duplicate `Link:` entries (last write wins).
  const indexLinkHeaders: [string, string][] = [
    [
      "Link",
      [
        '</llms.txt>; rel="describedby"; type="text/markdown"',
        '</.well-known/agent-skills/index.json>; rel="describedby"; type="application/json"',
        '</sitemap.xml>; rel="sitemap"; type="application/xml"',
      ].join(", "),
    ],
  ];

  const padLen = Math.max(
    ...targetFiles.map((f) => f.length),
    "agent-skills/index.json".length,
  );
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

    let target: DeployTarget;
    try {
      const raw = readFileSync(join(TOOLS_DIR, file), "utf-8");
      if (file.endsWith(".html")) {
        const slug = basename(file, ".html");
        const mdPath = mdNegotiationPaths.get(slug);
        // The index responder also gets the RFC 8288 Link headers pointing
        // at the discovery surfaces; per-tool responders don't need them
        // because the index already advertises everything an agent needs.
        const extraHeaders = slug === "index" ? indexLinkHeaders : undefined;
        target = await buildHtmlTarget(file, raw, TOOLS_HOST, label, mdPath, extraHeaders);
      } else {
        target = buildSkillTarget(file, raw, TOOLS_HOST);
      }
    } catch (err) {
      console.log(`  ${label}  ${ANSI.red(`✗ build error: ${err}`)}`);
      failed++;
      continue;
    }

    const minifiedSize = Buffer.byteLength(target.body, "utf-8");
    let sizeInfo: string;
    if (target.originalSize === minifiedSize) {
      sizeInfo = `${formatSize(target.originalSize)}`;
    } else {
      const savedPct = (
        ((target.originalSize - minifiedSize) / target.originalSize) *
        100
      ).toFixed(1);
      sizeInfo = `${formatSize(target.originalSize)} -> ${formatSize(minifiedSize)} ${ANSI.dim(`(${savedPct}% saved)`)}`;
    }
    if (target.script) {
      sizeInfo += ` ${ANSI.dim(`+ script ${formatSize(Buffer.byteLength(target.script, "utf-8"))}`)}`;
    }
    if (target.bundleBytes) {
      for (const [name, bytes] of target.bundleBytes) {
        sizeInfo += ` ${ANSI.dim(`+ bundle ${name} ${formatSize(bytes)}`)}`;
      }
    }

    const result = await putResponder(API_DOMAIN, API_KEY, responderId, target);
    if (result.ok) {
      console.log(`  ${label}  ${sizeInfo}  ${ANSI.green("✓ deployed")}`);
      deployed++;
    } else {
      console.log(
        `  ${label}  ${sizeInfo}  ${ANSI.red(`✗ HTTP ${result.status}: ${result.body.slice(0, 200)}`)}`,
      );
      failed++;
    }
  }

  // Always (re)deploy the agent-discovery aggregates at the end so they
  // reflect the current set of deployable tools, even if the user only asked
  // for a subset above. This keeps the agent surface consistent with what is
  // actually live (no 404 .md links advertised, no stale sitemap entries).
  // Each one is gated on its own `_*` responder ID so the user can roll out
  // surfaces incrementally; missing IDs print a yellow warning and skip.
  const aggregates: {
    label: string;
    envKey: string;
    build: () => DeployTarget;
  }[] = [];
  if (toolMetas.length > 0) {
    aggregates.push({
      label: "llms.txt",
      envKey: "SECUTILS_HTML_APP_RESPONDER_ID_LLMS_TXT",
      build: () => buildLlmsTxtTarget(buildLlmsTxt(toolMetas, TOOLS_HOST)),
    });
  }
  // The robots.txt and sitemap.xml are useful even with zero deployed tools
  // (the sitemap will just contain the index + llms.txt + agent-skills entry),
  // so they're not gated on `toolMetas.length > 0`.
  aggregates.push({
    label: "robots.txt",
    envKey: "SECUTILS_HTML_APP_RESPONDER_ID_ROBOTS_TXT",
    build: () => buildRobotsTxtTarget(buildRobotsTxt(TOOLS_HOST)),
  });
  aggregates.push({
    label: "sitemap.xml",
    envKey: "SECUTILS_HTML_APP_RESPONDER_ID_SITEMAP_XML",
    build: () => buildSitemapXmlTarget(buildSitemapXml(toolMetas, TOOLS_HOST)),
  });
  if (toolMetas.length > 0) {
    aggregates.push({
      label: "agent-skills/index.json",
      envKey: "SECUTILS_HTML_APP_RESPONDER_ID_AGENT_SKILLS_INDEX",
      build: () =>
        buildAgentSkillsIndexTarget(
          buildAgentSkillsIndex(toolMetas, skillBodies, TOOLS_HOST),
        ),
    });
  }

  for (const agg of aggregates) {
    const label = agg.label.padEnd(padLen);
    const responderId = process.env[agg.envKey];
    if (!responderId) {
      console.log(
        `  ${label}  ${ANSI.yellow(`⚠ skipped (no responder ID, expected ${agg.envKey})`)}`,
      );
      skipped++;
      continue;
    }
    const target = agg.build();
    const result = await putResponder(API_DOMAIN, API_KEY, responderId, target);
    const sizeInfo = formatSize(target.originalSize);
    if (result.ok) {
      console.log(`  ${label}  ${sizeInfo}  ${ANSI.green("✓ deployed")}`);
      deployed++;
    } else {
      console.log(
        `  ${label}  ${sizeInfo}  ${ANSI.red(`✗ HTTP ${result.status}: ${result.body.slice(0, 200)}`)}`,
      );
      failed++;
    }
  }

  console.log();
  const parts = [`${deployed} deployed`];
  if (skipped > 0) parts.push(`${skipped} skipped`);
  if (failed > 0) parts.push(ANSI.red(`${failed} failed`));
  log(parts.join(", "));

  process.exit(failed > 0 ? 1 : 0);
}

main();
