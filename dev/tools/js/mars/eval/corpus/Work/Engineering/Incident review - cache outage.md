# Incident review: product pages slow for 47 minutes

**Severity**: 2. **Date**: January 22. **Impact**: product pages took 6 to 12 seconds to
load; checkout was not affected.

## Timeline (UTC)

| Time | Event |
|------|-------|
| 09:02 | A deploy changes the cache key format for product pages |
| 09:04 | The cache hit rate drops from 96% to 3% |
| 09:06 | Database CPU reaches 100%; page latency alerts fire |
| 09:15 | On-call engineer suspects the database and adds a read replica |
| 09:38 | Someone spots the cache key change in the deploy diff |
| 09:41 | The deploy is rolled back |
| 09:49 | Hit rate back to 95%; latency normal |

## Root cause

The new key included a request ID by mistake, so no two requests shared an entry and every
page view went to the database. Entries also had no expiry, so the cache filled with keys
that were never read again and evicted the useful ones.

## What went well

Checkout uses a separate cache and kept working. The rollback took three minutes once the
cause was known.

## Action items

- [ ] Alert on cache hit rate below 80% for five minutes.
- [ ] Add a test that two identical requests produce the same cache key.
- [ ] Give every cache entry a time to live; 15 minutes for product pages.
- [x] Add the cache dashboard link to the on-call runbook.

<!-- su-comments
[
  {
    "id": "d93a7f28",
    "quote": "suspects the database and adds a read replica",
    "comments": [
      { "id": "5c2e8b91", "author": "Ana", "at": "2026-01-23T10:00:00.000Z", "body": "We lost 23 minutes here. The runbook should say: check the cache hit rate first when the database is suddenly hot." }
    ]
  }
]
-->
