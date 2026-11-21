// Retire only the unique Live-bearing collection named by a reconciled private audit.
// Without --apply this is a plan. Never print raw IDs, cursors, or operator credentials.
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { setTimeout as delay } from "node:timers/promises";
import { operatorKey, validateTestOrigin } from "../e2e/mars-test-scopes.ts";

const [origin, path, flag, ...extra] = process.argv.slice(2);
if (
  !origin ||
  !path ||
  (flag !== undefined && flag !== "--apply") ||
  extra.length
)
  throw Error(
    "Usage: node --env-file=.env scripts/mars-retire-collection.ts <origin> <private-audit.json> [--apply]",
  );
validateTestOrigin(origin);
const key = operatorKey(origin);
if (!key) throw Error("Missing MARS operator credential");
const report = JSON.parse(readFileSync(path, "utf8"));
const auditTime = Date.parse(report.startedAt) / 1000;
if (
  !report.reconciled ||
  !report.summary?.dependencyScanComplete ||
  !Number.isFinite(auditTime) ||
  !Array.isArray(report.rows)
)
  throw Error(
    "A reconciled private audit with complete dependency data is required",
  );
const matches = [
  ...new Set(
    report.rows
      .filter((row) => row.collection && row.liveUntil > auditTime)
      .map((row) => row.collection),
  ),
];
if (
  matches.length !== 1 ||
  typeof matches[0] !== "string" ||
  !/^[a-f0-9]{64}$/.test(matches[0])
)
  throw Error("Audit must identify exactly one Live-bearing collection");
const fingerprint = matches[0];
const request = async (action: string, fields: object = {}) => {
  const result = await fetch(`${origin}/sync/retire-collection`, {
    method: "POST",
    redirect: "error",
    signal: AbortSignal.timeout(20_000),
    headers: {
      Authorization: `Bearer ${key}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ ...fields, action, fingerprint }),
  });
  if (!result.ok) throw Error(`${action} failed (HTTP ${result.status})`);
  return result.json();
};
let collection: string | undefined;
for (const action of ["resolve", "resolve-retired"]) {
  let after: string | null = null;
  for (let page = 0; page < 4096; page++) {
    const result = await request(action, { after });
    if (result.collection) collection = result.collection;
    if (result.complete) break;
    if (typeof result.cursor !== "string" || result.cursor === after)
      throw Error("Resolution cursor stalled");
    after = result.cursor;
  }
  if (collection) break;
}
if (
  !collection ||
  createHash("sha256")
    .update(`mars-storage-audit-v1:collection:${collection}`)
    .digest("hex") !== fingerprint
)
  throw Error("Deployed responder did not resolve the exact fingerprint");
const plan = await request("plan", { collection });
if (!plan.matched && !plan.fenced)
  throw Error(
    "Matching owner record is absent and no retirement is in progress",
  );
console.log(
  JSON.stringify({
    phase: "plan",
    ...plan,
    expectedCollectionRows: report.summary.cohorts.liveOrShared.entries,
  }),
);
if (flag === "--apply") {
  const { notBefore } = await request("begin", { collection });
  if (!Number.isSafeInteger(notBefore)) throw Error("Invalid drain deadline");
  console.log(
    JSON.stringify({
      phase: "fenced",
      notBefore: new Date(notBefore * 1000).toISOString(),
    }),
  );
  // Use the server deadline; a local clock slightly ahead must not start the sweep too early.
  await delay(Math.max(0, notBefore * 1000 - Date.now()) + 2_000);
  for (const action of ["global", "invitations", "records", "budget", "refs"]) {
    let after: string | null = null,
      removed = 0,
      released = 0,
      complete = false;
    for (let page = 0; page < 4096; page++) {
      const result = await request(action, { collection, after });
      removed += result.removed ?? 0;
      released += result.released ?? 0;
      if (result.complete) {
        complete = true;
        break;
      }
      if (
        !result.retry &&
        (typeof result.cursor !== "string" || result.cursor === after)
      )
        throw Error(`${action} cursor stalled`);
      after = result.cursor;
    }
    if (!complete)
      throw Error(
        `${action} exceeded the page limit. Rerun with the same audit to resume.`,
      );
    console.log(JSON.stringify({ phase: action, removed, released }));
  }
}
