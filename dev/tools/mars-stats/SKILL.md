---
name: mars-observatory
description: >-
  Read private MARS storage statistics and export usage history from the
  MARS Observatory at https://{{TOOLS_HOST}}/mars-stats. Use when the owner
  asks about MARS storage capacity, entry growth, expiration, or telemetry.
  A dashboard access token is required. Account API credentials stay server-side.
---

# MARS Observatory

The observatory shows aggregate storage statistics for the configured MARS responder.
It stores telemetry in its own responder, separate from MARS.
It does not read documents, short-link contents, or visitor identities.

## Open the dashboard

Get the owner's dashboard token from `SECUTILS_MARS_STATS_VIEW_TOKEN` in the local deployment settings.
Create this private link with Node.js:

```js
const token = process.env.SECUTILS_MARS_STATS_VIEW_TOKEN;
if (!token) throw new Error('Dashboard access token is missing');
console.log(`https://{{TOOLS_HOST}}/mars-stats#key=${encodeURIComponent(token)}`);
```

The page stores the token in this tab's session storage and removes the fragment.
Reloads in the same tab retain access.
The token grants read access to telemetry only, not to the account API.

## Read statistics

Send the dashboard token in an Authorization header.
Keep the token out of query strings, exports, and public messages.

```js
const token = process.env.SECUTILS_MARS_STATS_VIEW_TOKEN;
const response = await fetch('https://{{TOOLS_HOST}}/mars-stats?action=stats', {
  headers: { Authorization: `Bearer ${token}`, Accept: 'application/json' },
});
if (!response.ok) throw new Error(`Statistics request failed: ${response.status}`);
console.log(JSON.stringify(await response.json(), null, 2));
```

The response contains:

| Field | Meaning |
|---|---|
| `at` | Snapshot time, in Unix seconds |
| `storage.entries` | Number of live KV entries |
| `storage.totalBytes` | Total bytes of live entry values |
| `storage.maxEntries` | Entry quota for MARS |
| `storage.maxTotalBytes` | Byte quota for MARS |
| `storage.oldestCreatedAt` | Creation time of the oldest live entry, or null |
| `storage.nextExpiresAt` | Earliest live expiration time, or null |
| `responder.settingsSize` | Serialized settings size, including body and script |
| `responder.requestCount` | Requests in retained history, not cumulative traffic |
| `responder.requestsToTrack` | Retained-history limit |
| `responder.lastRequestedAt` | Time of the last retained request, or null |
| `responder.updatedAt` | Time of the last responder update |
| `responder.enabled` | Whether MARS is enabled |
| `intervalSec`, `retentionSec` | Sample interval and retention window |

Statistics use a sixty-second cache.
The page derives average entry size and quota percentages from the measured fields.
If a live read fails, the endpoint can return the last successful snapshot with `stale: true` and a sanitized `failure`.
The snapshot keeps its original `at` timestamp, and the page labels it as stale.

## Read history

Use `GET ?action=history&since=<unix-seconds>` with the same dashboard token.
The endpoint limits the range to the last thirty days.
Each response contains `columns`, `points`, and `cursor`.
Each point is an array whose positions match `columns`.

If `cursor` is not null, repeat the request with `&after=<cursor>`.
Each page contains at most 1,000 samples.
CSV and JSON exports include every loaded sample in the selected period.

## Collection and limits

The account's API tracker sends `POST ?action=sample` every thirty minutes.
Collector authentication uses the selected `MARS_STATS_API_KEY` secret.
The dashboard token cannot call this endpoint.
Read requests also collect a snapshot when the sixty-second cache expires.

The responder stores one immutable sample per thirty-minute bucket.
Samples expire after thirty days, so new history remains bounded at approximately 1,440 entries.
Earlier five-minute samples remain available until their original expiration.
The collector keeps one revision and sends no change notifications.
The tracker retries failed scheduled runs after one, two, and four minutes.
Three retries are the limit, and the normal schedule remains every thirty minutes.
Retries use the same thirty-minute storage bucket, so a successful retry fills that sample without duplicates.
The collector returns an error for a failed measurement, never a cached measurement as a new sample.

Errors contain sanitized `code`, `stage`, and `retryable` fields instead of raw upstream responses or credentials.
Scheduler retries depend on the job settings, not the `retryable` response field.
Optional cache-write failures do not invalidate a successfully stored sample.

History starts when collection begins.
Dashed chart lines connect observations across collection gaps.
Shaded bands and hover text identify periods without measurements.
The chart adds no synthetic samples, and exports contain measured points only.
Collection gaps are not zero usage, and net changes are not counts of created or deleted links.
Entries include encrypted payloads, deletion-token hashes, and temporary rate-limit markers.
The storage API does not expose per-prefix counts, link sizes, runtime heap usage, or cumulative traffic.

## Deployment

The HTML and companion skill use separate responders at `/mars-stats` and `/mars-stats.md` on the tools host.
Both locations use exact path matching.
Their IDs use `SECUTILS_HTML_APP_RESPONDER_ID_MARS_STATS` and `SECUTILS_HTML_APP_RESPONDER_ID_MARS_STATS_MD`.
The collector ID uses `SECUTILS_MARS_STATS_TRACKER_ID`.

Deployment grants the HTML responder access to the selected `MARS_STATS_API_KEY` secret.
It injects the MARS responder ID, API origin, and dashboard-token hash into the server script only.
It never injects either plaintext credential into the HTML.
The observatory is unpromoted and absent from discovery indexes.
