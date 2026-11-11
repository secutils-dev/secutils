# Read-only storage audit

`POST /sync/storage-audit` measures MARS storage for the operator.
It uses the existing `MARS_STATS_API_KEY` secret.
Participant and test-scope credentials cannot authorize it.
The audit performs no KV writes, deletes, or lease renewals.

## Run

From the repository root, run:
Replace `{{MARS_HOST}}` with the configured MARS host.

```bash
node --env-file=.env scripts/mars-storage-audit.ts https://{{MARS_HOST}}

# Optional private report. The file must not already exist.
node --env-file=.env scripts/mars-storage-audit.ts https://{{MARS_HOST}} private-report.json
```

The CLI accepts only the configured MARS origin or a loopback preview.
It rejects redirects and keeps the operator credential in the Node process.
Private report files use mode `0600`.
Console output contains aggregate counts and bytes, not collection identifiers.

## Measurements and privacy

Each page lists at most 32 keys without values, then reads each value separately.
One request uses at most 33 KV reads.
Sequential reads avoid a large batch of ciphertexts in the sandbox heap.

Sizes measure decoded value bytes, like the storage API.
They do not measure reserved capacity, PostgreSQL rows, or indexes.
Only small invitation, membership, and quota metadata records are parsed.
The audit does not decrypt document ciphertext.

Response rows contain categories, sizes, expiry timestamps, and hashed join keys.
They contain no stored values, document names, document content, encryption keys, bearer tokens, or deletion capabilities.
The private pagination cursor encodes a storage key.
POST keeps the cursor out of URLs. The Node client keeps the cursor in memory, outside reports and console output.

## Conservative Sync-only estimate

An unexpired Live invitation protects its complete collection, even before a recipient claims the link.
An unexpired document-scoped grant also protects its complete collection.
A document-scoped grant without an expiry protects its collection indefinitely.
The estimate retains revoked or unusable grants until expiry.
Mixed Sync/Live collections remain intact, including their Sync-only documents and shared metadata.

Snapshots, test records, and unclassified records remain separate.
Every global quota-ledger record remains protected, even with a Sync-only owner.
The report shows those Sync-associated global quota bytes separately.
They are not part of the conservative candidate total.

Malformed membership metadata protects its collection.
Unreadable invitation ownership blocks all Sync-only classification.
The candidate total measures gross value bytes in collections without a detected Live dependency.
It is a size estimate, not a deletion plan.
Safe retirement can require retained fences or free-quota records.

## Consistency

The scan is paginated, not transactional.
Concurrent uploads, expiry, or maintenance can change the totals during a scan.
The CLI compares storage totals before and after the scan with the measured sum.
`reconciled: true` means that all three entry counts and byte totals match.
It does not exclude equal-size updates during the scan.
Live dependencies use the start time of the scan, so expiry during the scan cannot increase the candidate total.

The existing `/sync/telemetry` response remains unchanged.
The audit does not change ordinary Sync, Live-link, or snapshot behavior.
