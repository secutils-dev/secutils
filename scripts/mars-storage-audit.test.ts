import assert from "node:assert/strict";
import { test } from "node:test";
import { summarizeStorageAudit } from "./mars-storage-audit.ts";
import type { StorageAuditRow } from "../dev/tools/mars/src/sync/storage-audit.ts";

const collection = "a".repeat(64),
  shared = "b".repeat(64),
  invitation = "c".repeat(64);
const row = (
  kind: string,
  bytes: number,
  extra: Partial<StorageAuditRow> = {},
): StorageAuditRow => ({ kind, bytes, expiresAt: null, ...extra });

test("unclaimed Live invitations protect their whole collection and supporting records", () => {
  const report = summarizeStorageAudit(
    [
      row("sync-payload", 100, { collection }),
      row("sync-payload", 200, { collection: shared }),
      row("sync-invitation", 10, {
        collection: shared,
        invitation,
        liveSource: "invitation",
        liveUntil: 200,
      }),
      row("sync-invitation-support", 20, { invitation }),
      row("snapshot-payload", 30),
      row("sync-global-quota-ledger", 40, { collection }),
    ],
    100,
  );
  assert.deepEqual(report.cohorts.syncOnly, {
    entries: 1,
    bytes: 100,
    permanentEntries: 1,
    permanentBytes: 100,
    collections: 1,
  });
  assert.equal(report.cohorts.liveOrShared.bytes, 230);
  assert.equal(report.cohorts.snapshots.bytes, 30);
  assert.equal(report.cohorts.globalQuota.bytes, 40);
  assert.equal(report.syncOnlyGlobalQuota.bytes, 40);
  assert.equal(
    Object.values(report.cohorts).reduce((sum, group) => sum + group.bytes, 0),
    report.total.bytes,
  );
});

test("unexpired Live grants protect data even without an invitation row", () => {
  const report = summarizeStorageAudit(
    [
      row("sync-payload", 100, { collection }),
      row("sync-member-grant", 10, {
        collection,
        liveSource: "grant",
        liveUntil: 200,
      }),
      row("sync-payload", 200, { collection: shared }),
      row("sync-member-grant", 10, {
        collection: shared,
        liveSource: "grant",
        liveUntil: 99,
      }),
    ],
    100,
  );
  assert.equal(report.cohorts.liveOrShared.bytes, 110);
  assert.equal(report.cohorts.syncOnly.bytes, 210);
});

test("missing invitation metadata fails closed for every candidate collection", () => {
  const report = summarizeStorageAudit([
    row("sync-payload", 100, { collection }),
    row("sync-invitation", 10, { invitation, problem: "invalid-metadata" }),
  ]);
  assert.equal(report.dependencyScanComplete, false);
  assert.equal(report.cohorts.syncOnly.bytes, 0);
  assert.equal(report.cohorts.uncertain.bytes, 100);
});

test("a malformed member protects its collection without hiding other categories", () => {
  const report = summarizeStorageAudit([
    row("sync-payload", 100, { collection }),
    row("sync-member-grant", 10, { collection, problem: "invalid-metadata" }),
    row("sync-payload", 200, { collection: shared }),
    row("test-audit", 20),
  ]);
  assert.equal(report.cohorts.uncertain.bytes, 110);
  assert.equal(report.cohorts.syncOnly.bytes, 200);
  assert.equal(report.cohorts.tests.bytes, 20);
});

test("an unknown Sync namespace blocks all Sync-only candidates", () => {
  const report = summarizeStorageAudit([
    row("sync-payload", 100, { collection }),
    row("sync-other", 10),
    row("sync-global-quota-ledger", 20, { collection }),
  ]);
  assert.equal(report.dependencyScanComplete, false);
  assert.equal(report.cohorts.syncOnly.bytes, 0);
  assert.equal(report.cohorts.uncertain.bytes, 100);
  assert.equal(report.syncOnlyGlobalQuota.bytes, 0);
});
