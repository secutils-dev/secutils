// Read-only MARS storage sizing. Values, document text, encryption keys, and credentials are never returned.
// Pagination cursors stay in memory; private report rows contain hashed join keys only.
import { writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { operatorKey, validateTestOrigin } from "../e2e/mars-test-scopes.ts";
import type {
  StorageAuditPage,
  StorageAuditRow,
} from "../dev/tools/mars/src/sync/storage-audit.ts";

const MAX_PAGES = 4096;
const REQUEST_TIMEOUT_MS = 20000;
const HASH = /^[a-f0-9]{64}$/;
const KINDS = new Set([
  "other",
  "test-audit",
  "test-data",
  "snapshot-payload",
  "snapshot-delete-token",
  "snapshot-rate-limit",
  "sync-collection",
  "sync-payload",
  "sync-checkpoint-header",
  "sync-document-metadata",
  "sync-member-grant",
  "sync-wrapped-key",
  "sync-key-epoch",
  "sync-revocation-fence",
  "sync-deletion-fence",
  "sync-collection-metadata",
  "sync-invitation",
  "sync-invitation-support",
  "sync-global-quota-ledger",
  "sync-collection-quota-ledger",
  "sync-reservation-reference",
  "sync-creation-rate-limit",
  "sync-other",
]);
const COHORTS = [
  "syncOnly",
  "liveOrShared",
  "uncertain",
  "globalQuota",
  "snapshots",
  "tests",
  "unattributedSync",
  "other",
] as const;
type Cohort = (typeof COHORTS)[number];
type Usage = {
  entries: number;
  bytes: number;
  permanentEntries: number;
  permanentBytes: number;
};
const empty = (): Usage => ({
  entries: 0,
  bytes: 0,
  permanentEntries: 0,
  permanentBytes: 0,
});
const add = (usage: Usage, row: StorageAuditRow) => {
  usage.entries++;
  usage.bytes += row.bytes;
  if (row.expiresAt === null) {
    usage.permanentEntries++;
    usage.permanentBytes += row.bytes;
  }
};

/** Aggregate measured value bytes. Preserve whole Live-bearing or uncertain collections and every global quota row. */
export function summarizeStorageAudit(
  input: StorageAuditRow[],
  now = Math.floor(Date.now() / 1000),
) {
  const invitations = new Map(
    input
      .filter(
        (row) =>
          row.invitation && row.collection && row.kind === "sync-invitation",
      )
      .map((row) => [row.invitation!, row.collection!]),
  );
  const rows = input.map((row) => ({
    ...row,
    collection:
      row.collection ??
      (row.invitation ? invitations.get(row.invitation) : undefined),
  }));
  const collections = new Set(
    rows.flatMap((row) => (row.collection ? [row.collection] : [])),
  );
  const liveCollections = new Set(
    rows
      .filter((row) => row.collection && (row.liveUntil ?? 0) > now)
      .map((row) => row.collection!),
  );
  const uncertainCollections = new Set(
    rows
      .filter(
        (row) =>
          row.problem &&
          row.collection &&
          row.kind !== "sync-global-quota-ledger",
      )
      .map((row) => row.collection!),
  );
  const dependencyScanComplete = !rows.some(
    (row) =>
      row.kind === "sync-other" ||
      (row.kind === "sync-invitation" && row.problem && !row.collection),
  );
  const cohorts = Object.fromEntries(
    COHORTS.map((kind) => [kind, { ...empty(), collections: 0 }]),
  ) as Record<Cohort, Usage & { collections: number }>;
  const cohortCollections = new Map<Cohort, Set<string>>(
    COHORTS.map((kind) => [kind, new Set()]),
  );
  const byKind = new Map<string, Usage>();
  const total = empty(),
    syncOnlyGlobalQuota = empty();
  for (const row of rows) {
    add(total, row);
    const kind = byKind.get(row.kind) ?? empty();
    add(kind, row);
    byKind.set(row.kind, kind);
    let cohort: Cohort;
    if (row.kind.startsWith("snapshot-")) cohort = "snapshots";
    else if (row.kind.startsWith("test-")) cohort = "tests";
    else if (row.kind === "sync-global-quota-ledger") {
      cohort = "globalQuota";
      if (
        dependencyScanComplete &&
        row.collection &&
        !liveCollections.has(row.collection) &&
        !uncertainCollections.has(row.collection)
      )
        add(syncOnlyGlobalQuota, row);
    } else if (row.kind.startsWith("sync-")) {
      if (!row.collection) cohort = "unattributedSync";
      else if (
        !dependencyScanComplete ||
        uncertainCollections.has(row.collection)
      )
        cohort = "uncertain";
      else
        cohort = liveCollections.has(row.collection)
          ? "liveOrShared"
          : "syncOnly";
    } else cohort = "other";
    add(cohorts[cohort], row);
    if (row.collection) cohortCollections.get(cohort)!.add(row.collection);
  }
  for (const kind of COHORTS)
    cohorts[kind].collections = cohortCollections.get(kind)!.size;
  return {
    total,
    collectionIdentifiersObserved: collections.size,
    dependencyScanComplete,
    metadataProblems: rows.filter((row) => row.problem).length,
    unexpiredLiveInvitations: rows.filter(
      (row) => row.liveSource === "invitation" && (row.liveUntil ?? 0) > now,
    ).length,
    unexpiredLiveGrants: rows.filter(
      (row) => row.liveSource === "grant" && (row.liveUntil ?? 0) > now,
    ).length,
    cohorts,
    syncOnlyGlobalQuota,
    byKind: [...byKind]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([kind, usage]) => ({ kind, ...usage })),
    notes: [
      "Sync-only is a conservative whole-collection estimate, not a deletion plan.",
      "Any unexpired Live invitation or grant protects its complete collection, even if revoked or otherwise unusable.",
      "All snapshot records, uncertain records, and shared global quota ledgers are preserved.",
      "Candidate sizes are gross value bytes. Safe retirement can require retained fences or free-quota records.",
      "This is not a transactional snapshot; compare totals before and after the scan.",
    ],
  };
}

function validatedRow(value: StorageAuditRow): StorageAuditRow {
  if (
    !value ||
    !KINDS.has(value.kind) ||
    !Number.isSafeInteger(value.bytes) ||
    value.bytes < 0 ||
    (value.expiresAt !== null &&
      (!Number.isSafeInteger(value.expiresAt) || value.expiresAt < 0))
  )
    throw new Error("Invalid audit row");
  const row: StorageAuditRow = {
    kind: value.kind,
    bytes: value.bytes,
    expiresAt: value.expiresAt,
  };
  for (const field of ["collection", "invitation", "document"] as const) {
    if (value[field] !== undefined) {
      if (!HASH.test(value[field]!)) throw new Error("Invalid audit join key");
      row[field] = value[field];
    }
  }
  for (const field of ["createdAt", "liveUntil"] as const) {
    if (value[field] !== undefined) {
      if (!Number.isSafeInteger(value[field]) || value[field]! < 0)
        throw new Error("Invalid audit timestamp");
      row[field] = value[field];
    }
  }
  if (value.liveSource !== undefined) {
    if (!["invitation", "grant"].includes(value.liveSource))
      throw new Error("Invalid Live source");
    row.liveSource = value.liveSource;
  }
  if (value.invitationMode !== undefined) {
    if (!["live", "pair"].includes(value.invitationMode))
      throw new Error("Invalid invitation mode");
    row.invitationMode = value.invitationMode;
  }
  if (value.problem !== undefined) {
    if (!["invalid-metadata", "oversized-metadata"].includes(value.problem))
      throw new Error("Invalid audit diagnostic");
    row.problem = value.problem;
  }
  return row;
}

async function main() {
  const args = process.argv.slice(2);
  if (args.includes("--help")) {
    console.log(
      "Usage: node --env-file=.env scripts/mars-storage-audit.ts <configured-MARS-origin> [private-report.json]",
    );
    console.log(
      "Read-only. Prints aggregate counts and bytes. Optional private output includes hashed join keys, never values or credentials.",
    );
    return;
  }
  const [origin, output] = args;
  if (!origin || args.length > 2)
    throw new Error(
      "Provide the configured MARS origin and an optional private report path.",
    );
  const local = validateTestOrigin(origin),
    key = operatorKey(origin);
  if (!key) throw new Error("The MARS operator credential is missing.");
  const storage = async () => {
    if (local) return null;
    const api = new URL(
      process.env.SECUTILS_HTML_APP_API_DOMAIN || "https://secutils.dev",
    );
    if (
      api.protocol !== "https:" ||
      api.username ||
      api.password ||
      api.pathname !== "/"
    )
      throw new Error("Invalid API origin");
    const id = process.env.SECUTILS_HTML_APP_RESPONDER_ID_MARS;
    if (!id) throw new Error("The MARS responder ID is missing.");
    const response = await fetch(
      `${api.origin}/api/webhooks/responders/${id}/_storage`,
      {
        redirect: "error",
        signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
        headers: {
          Authorization: `Bearer ${process.env.SECUTILS_HTML_APP_API_KEY}`,
        },
      },
    );
    if (!response.ok)
      throw new Error(
        `Storage reconciliation failed (HTTP ${response.status}).`,
      );
    const data = await response.json();
    if (
      !Number.isSafeInteger(data.entries) ||
      !Number.isSafeInteger(data.totalBytes)
    )
      throw new Error("Invalid storage statistics");
    return {
      entries: data.entries as number,
      bytes: data.totalBytes as number,
    };
  };
  const startedAt = new Date().toISOString(),
    before = await storage(),
    rows: StorageAuditRow[] = [];
  let after: string | null = null,
    missing = 0,
    pages = 0;
  for (; pages < MAX_PAGES; pages++) {
    const response = await fetch(`${origin}/sync/storage-audit`, {
      method: "POST",
      redirect: "error",
      signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
      headers: {
        Authorization: `Bearer ${key}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ after }),
    });
    if (!response.ok)
      throw new Error(`Storage audit failed (HTTP ${response.status}).`);
    const page = (await response.json()) as StorageAuditPage;
    if (
      page.v !== 1 ||
      !Array.isArray(page.rows) ||
      page.rows.length > 32 ||
      !Number.isSafeInteger(page.missing)
    )
      throw new Error("Invalid audit page");
    rows.push(...page.rows.map(validatedRow));
    missing += page.missing;
    if (page.complete) {
      pages++;
      break;
    }
    if (typeof page.cursor !== "string" || page.cursor === after)
      throw new Error("Audit cursor did not advance");
    after = page.cursor;
  }
  if (pages >= MAX_PAGES) throw new Error("Audit reached its page limit");
  const current = await storage(),
    summary = summarizeStorageAudit(
      rows,
      Math.floor(Date.parse(startedAt) / 1000),
    );
  const report = {
    startedAt,
    finishedAt: new Date().toISOString(),
    pages,
    missing,
    before,
    after: current,
    reconciled:
      !!before &&
      !!current &&
      before.entries === current.entries &&
      before.bytes === current.bytes &&
      current.entries === summary.total.entries &&
      current.bytes === summary.total.bytes,
    summary,
  };
  if (output)
    writeFileSync(
      resolve(output),
      JSON.stringify({ ...report, rows }, null, 2),
      { mode: 0o600, flag: "wx" },
    );
  console.log(JSON.stringify(report, null, 2));
}

if (import.meta.main) await main();
