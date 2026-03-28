#!/usr/bin/env node

// Bundle size analyzer for Parcel builds.
import { execSync } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

// ── Types ────────────────────────────────────────────────

interface Sizes {
  raw: number;
  gzip: number;
  brotli: number;
}

interface BundleEntry extends Sizes {
  category: string;
}

interface TrackedBundle {
  name: string;
  pattern: string;
  category: string;
}

interface Config {
  thresholds: {
    individual: number;
    total: number;
    category: number;
  };
  trackedBundles: TrackedBundle[];
}

interface Report {
  timestamp: string;
  commitSha: string;
  commitMessage: string;
  categories: Record<string, Sizes>;
  bundles: Record<string, BundleEntry>;
  otherChunkCount: number;
}

interface FileEntry extends Sizes {
  relPath: string;
}

// ── Constants ────────────────────────────────────────────

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');
const DIST = join(ROOT, 'dist');
const BUNDLESIZE_DIR = join(ROOT, '.bundlesize');
const CONFIG_PATH = join(BUNDLESIZE_DIR, 'config.json');
const HISTORY_PATH = join(BUNDLESIZE_DIR, 'history.jsonl');

// ── Helpers ──────────────────────────────────────────────

function formatBytes(bytes: number) {
  if (bytes === 0) {
    return '0 B';
  }
  const units = ['B', 'KB', 'MB', 'GB'];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** i;
  return `${value.toFixed(i === 0 ? 0 : 2)} ${units[i]}`;
}

function pctChange(current: number, previous: number) {
  return previous === 0 ? (current === 0 ? 0 : 100) : ((current - previous) / previous) * 100;
}

function formatPct(pct: number) {
  return `${pct > 0 ? '+' : ''}${pct.toFixed(1)}%`;
}

/** Simple glob match supporting only `*` wildcard segments. */
function globMatch(pattern: string, filename: string) {
  return new RegExp('^' + pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*') + '$').test(filename);
}

/** Recursively walk a directory, returning relative paths of files. */
function walkDir(dir: string, base: string = dir) {
  const results: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      results.push(...walkDir(full, base));
    } else {
      results.push(relative(base, full));
    }
  }
  return results;
}

function getGitInfo() {
  // Walk up to find the git root (the webui is a subdirectory, not a standalone repo).
  try {
    const gitRoot = execSync('git -C . rev-parse --show-toplevel', {
      encoding: 'utf8',
      cwd: ROOT,
      stdio: ['pipe', 'pipe', 'pipe'],
    }).trim();
    const sha = execSync('git rev-parse HEAD', {
      encoding: 'utf8',
      cwd: gitRoot,
      stdio: ['pipe', 'pipe', 'pipe'],
    }).trim();
    const msg = execSync('git log -1 --format=%s', {
      encoding: 'utf8',
      cwd: gitRoot,
      stdio: ['pipe', 'pipe', 'pipe'],
    }).trim();
    return { sha, msg };
  } catch {
    return { sha: 'unknown', msg: 'unknown' };
  }
}

function sizeOf(filePath: string) {
  try {
    return statSync(filePath).size;
  } catch {
    return 0;
  }
}

// ── Main ─────────────────────────────────────────────────

function loadConfig(): Config {
  const defaults: Config = {
    thresholds: { individual: 10, total: 5, category: 8 },
    trackedBundles: [],
  };
  if (!existsSync(CONFIG_PATH)) {
    return defaults;
  }
  try {
    return { ...defaults, ...JSON.parse(readFileSync(CONFIG_PATH, 'utf8')) };
  } catch {
    console.warn('⚠ Could not parse config.json, using defaults');
    return defaults;
  }
}

function scanBundles(config: Config): {
  bundles: Record<string, BundleEntry>;
  categories: Record<string, Sizes>;
  otherChunkCount: number;
} {
  if (!existsSync(DIST)) {
    console.error('Error: dist/ directory not found. Run `npm run build` first.');
    process.exit(1);
  }

  const allFiles = walkDir(DIST);
  const assets = allFiles.filter(
    (f) => (f.endsWith('.js') || f.endsWith('.css')) && !f.endsWith('.map') && !f.endsWith('.br') && !f.endsWith('.gz'),
  );

  // Collect raw sizes with compressed variants.
  const entries: FileEntry[] = assets.map((relPath) => {
    const absPath = join(DIST, relPath);
    const raw = sizeOf(absPath);
    const gzip = sizeOf(absPath + '.gz');
    const brotli = sizeOf(absPath + '.br');
    return { relPath, raw, gzip, brotli };
  });

  // Categorize each file. Parcel code-splits routes into multiple chunks with the same name
  // prefix (e.g., responders.{hash1}.js, responders.{hash2}.js). We sum all of them together
  // since they're all loaded as part of the same feature. Use `build:analyze` (which cleans
  // dist/ first) to avoid counting stale files from previous builds.
  const bundles: Record<string, BundleEntry> = {};
  const uncategorized: FileEntry[] = [];

  for (const entry of entries) {
    const filename = entry.relPath.split('/').pop()!;
    let matched = false;

    // Check tracked bundles first.
    for (const tb of config.trackedBundles) {
      if (globMatch(tb.pattern, filename)) {
        const key = tb.name;
        if (!bundles[key]) {
          bundles[key] = { raw: 0, gzip: 0, brotli: 0, category: tb.category };
        }
        bundles[key].raw += entry.raw;
        bundles[key].gzip += entry.gzip;
        bundles[key].brotli += entry.brotli;
        matched = true;
        break;
      }
    }

    if (matched) {
      continue;
    }

    // Check monaco workers: tools/monaco/*.js or *.worker.*.js at root.
    const isMonacoEntry = entry.relPath.startsWith('tools/monaco/') && entry.relPath.endsWith('.js');
    const isWorkerChunk = !entry.relPath.includes('/') && /\.worker\.[a-f0-9]+\.js$/.test(filename);

    if (isMonacoEntry || isWorkerChunk) {
      // Extract logical name: "ts.worker" from "ts.worker.84a05b32.js" or "ts.worker.js".
      const logicalName = filename.replace(/\.[a-f0-9]{8}\.js$/, '').replace(/\.js$/, '');
      const key = `worker:${logicalName}`;
      if (!bundles[key]) {
        bundles[key] = { raw: 0, gzip: 0, brotli: 0, category: 'monaco-workers' };
      }
      bundles[key].raw += entry.raw;
      bundles[key].gzip += entry.gzip;
      bundles[key].brotli += entry.brotli;
      continue;
    }

    // Everything else is uncategorized.
    uncategorized.push(entry);
  }

  // Aggregate uncategorized into an "other" bundle entry.
  const otherSizes = uncategorized.reduce<Sizes>(
    (acc, e) => ({ raw: acc.raw + e.raw, gzip: acc.gzip + e.gzip, brotli: acc.brotli + e.brotli }),
    { raw: 0, gzip: 0, brotli: 0 },
  );
  bundles['other'] = { ...otherSizes, category: 'other' };

  // Build category totals.
  const categories: Record<string, Sizes> = {};
  for (const [, b] of Object.entries(bundles)) {
    if (!categories[b.category]) {
      categories[b.category] = { raw: 0, gzip: 0, brotli: 0 };
    }
    categories[b.category].raw += b.raw;
    categories[b.category].gzip += b.gzip;
    categories[b.category].brotli += b.brotli;
  }

  // Total across all categories.
  categories['total'] = Object.values(categories).reduce<Sizes>(
    (acc, c) => ({ raw: acc.raw + c.raw, gzip: acc.gzip + c.gzip, brotli: acc.brotli + c.brotli }),
    { raw: 0, gzip: 0, brotli: 0 },
  );

  return { bundles, categories, otherChunkCount: uncategorized.length };
}

function buildReport(config: Config): Report {
  const { bundles, categories, otherChunkCount } = scanBundles(config);
  const { sha, msg } = getGitInfo();

  return {
    timestamp: new Date().toISOString(),
    commitSha: sha,
    commitMessage: msg,
    categories,
    bundles,
    otherChunkCount,
  };
}

function loadHistory(): Report[] {
  if (!existsSync(HISTORY_PATH)) {
    return [];
  }
  const content = readFileSync(HISTORY_PATH, 'utf8').trim();
  if (!content) {
    return [];
  }
  return content.split('\n').map((line) => JSON.parse(line) as Report);
}

function appendReport(report: Report): void {
  mkdirSync(BUNDLESIZE_DIR, { recursive: true });
  appendFileSync(HISTORY_PATH, JSON.stringify(report) + '\n');
}

function printTable(report: Report, previous: Report | null, config: Config): void {
  const shaShort = report.commitSha.slice(0, 7);
  const line = '─'.repeat(72);

  console.log('');
  console.log(`Bundle Size Report - ${shaShort} (${report.commitMessage})`);
  console.log(line);
  console.log('Bundle'.padEnd(28) + 'Raw'.padStart(11) + 'Brotli'.padStart(11) + 'Change'.padStart(11) + '  Status');
  console.log(line);

  const warnings: string[] = [];

  // Print bundles.
  for (const [name, sizes] of Object.entries(report.bundles)) {
    const prevSizes = previous?.bundles?.[name];
    const change = prevSizes ? pctChange(sizes.brotli, prevSizes.brotli) : null;
    const changeStr = change !== null ? formatPct(change) : 'new';

    let status = '';
    if (change !== null && Math.abs(change) > config.thresholds.individual) {
      if (change > 0) {
        status = '⚠ WARN';
        warnings.push(`"${name}" increased by ${formatPct(change)} (threshold: ${config.thresholds.individual}%)`);
      }
    }

    const displayName = name === 'other' ? `other (${report.otherChunkCount} chunks)` : name;
    console.log(
      displayName.padEnd(28) +
        formatBytes(sizes.raw).padStart(11) +
        formatBytes(sizes.brotli).padStart(11) +
        changeStr.padStart(11) +
        (status ? `  ${status}` : ''),
    );
  }

  console.log(line);
  console.log('CATEGORIES');

  // Print category totals.
  for (const [cat, sizes] of Object.entries(report.categories)) {
    if (cat === 'total') {
      continue;
    }
    const prevSizes = previous?.categories?.[cat];
    const change = prevSizes ? pctChange(sizes.brotli, prevSizes.brotli) : null;
    const changeStr = change !== null ? formatPct(change) : 'new';

    let status = '';
    if (change !== null && Math.abs(change) > config.thresholds.category) {
      if (change > 0) {
        status = '⚠ WARN';
        warnings.push(
          `Category "${cat}" increased by ${formatPct(change)} (threshold: ${config.thresholds.category}%)`,
        );
      }
    }

    console.log(
      cat.padEnd(28) +
        formatBytes(sizes.raw).padStart(11) +
        formatBytes(sizes.brotli).padStart(11) +
        changeStr.padStart(11) +
        (status ? `  ${status}` : ''),
    );
  }

  console.log(line);

  // Total.
  {
    const sizes = report.categories['total'];
    const prevSizes = previous?.categories?.['total'];
    const change = prevSizes ? pctChange(sizes.brotli, prevSizes.brotli) : null;
    const changeStr = change !== null ? formatPct(change) : 'new';

    let status = change !== null && change > config.thresholds.total ? '⚠ WARN' : '✓';
    if (change !== null && change > config.thresholds.total) {
      warnings.push(`Total increased by ${formatPct(change)} (threshold: ${config.thresholds.total}%)`);
    }
    if (change === null) {
      status = '';
    }

    console.log(
      'TOTAL'.padEnd(28) +
        formatBytes(sizes.raw).padStart(11) +
        formatBytes(sizes.brotli).padStart(11) +
        changeStr.padStart(11) +
        `  ${status}`,
    );
  }

  console.log(line);

  if (!previous) {
    console.log('\nFirst build recorded - no comparison available.');
  }

  if (warnings.length > 0) {
    console.log('');
    for (const w of warnings) {
      console.log(`⚠ WARNING: ${w}`);
    }
  }

  console.log('');
}

function sizesEqual(a: Report, b: Report): boolean {
  return (
    JSON.stringify(a.bundles) === JSON.stringify(b.bundles) &&
    JSON.stringify(a.categories) === JSON.stringify(b.categories) &&
    a.otherChunkCount === b.otherChunkCount
  );
}

// ── Entry point ──────────────────────────────────────────

const config = loadConfig();
const report = buildReport(config);
const history = loadHistory();

// Idempotency: skip if bundle sizes haven't changed since the last recording.
const latest = history.at(-1);
if (latest && sizesEqual(report, latest)) {
  console.log('Bundle sizes unchanged since last recording, skipping.');
  printTable(report, history.at(-2) ?? null, config);
  process.exit(0);
}

appendReport(report);

const previous = latest ?? null;
printTable(report, previous, config);
