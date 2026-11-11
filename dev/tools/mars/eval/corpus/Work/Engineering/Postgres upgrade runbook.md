# Postgres 15 to 17 upgrade runbook

The primary database runs Postgres 15 on a managed instance with one read replica. This
runbook upgrades both to Postgres 17 with about ten minutes of write downtime.

## Before the day

1. Restore last night's snapshot to a scratch instance and run the upgrade there first.
2. Run the test suite against the scratch instance.
3. Check extensions: `pg_stat_statements` and `pgcrypto` are supported on 17; `plv8` is not
   and must be dropped first.
4. Announce the maintenance window to customers 72 hours ahead.

## Upgrade steps

```bash
# 1. Stop the workers and put the app in read-only mode
kubectl scale deploy/worker --replicas=0
kubectl set env deploy/api READ_ONLY=true

# 2. Take a manual snapshot
cloud db snapshot create primary --name pre-upgrade

# 3. Upgrade in place
cloud db upgrade primary --version 17

# 4. Refresh planner statistics
psql "$DATABASE_URL" -c 'ANALYZE;'
```

Then turn read-only mode off, scale the workers back up, and watch the error rate and the
p95 latency for 30 minutes.

## Rollback

If the error rate stays above 1% for ten minutes, restore the `pre-upgrade` snapshot to a
new instance and point the app at it. Writes made after the snapshot are lost, which is why
the app is read-only during the upgrade.

## After the upgrade

- Recreate the read replica from the new primary.
- Rebuild indexes that the release notes flag as affected by collation changes.
- Remove the scratch instance.

<!-- su-comments
[
  {
    "id": "a4f9e2c1",
    "quote": "Rebuild indexes that the release notes flag",
    "comments": [
      { "id": "c6b1d7a2", "author": "Dana", "at": "2026-04-03T10:11:00.000Z", "body": "The ICU collation change touches the text indexes on the users table. Plan 20 minutes for REINDEX CONCURRENTLY." }
    ]
  }
]
-->
