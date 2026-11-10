# Responder-only sync

Status: release checks passed on 2026-11-08. Deployment is approved.
The normal MARS entry loads the connection manager for enrollment or connected content.
Local preview and deployment assemble the same responder through `dev/tools/mars-sync-assets.ts`.

## Implemented foundations

- `replica.ts` isolates Yjs body operations, local undo, and relative anchors.
- `session.ts` keeps the displayed editor's CRDT separate from received state. Typing during a remote transaction preserves both edits.
- `manager.ts` owns enrollment, pairing, folder manifests, live sessions, connection details, recovery, and disconnection.
- `reviews.ts` separates signed review operations from body state. Concurrent comment versions remain visible.
- `crypto.ts` uses native P-256 signing/key exchange, HKDF-SHA256, and AES-256-GCM.
- `durable.ts` saves local operations before encryption. It saves the exact ciphertext before upload.
- `transport.ts` uses short polls, header credentials, bounded responses, backoff, and explicit Retry.
- `responder.ts` provides account-free scopes, grants, immutable checkpoints, acknowledgement, inactivity renewal, and revocation fences.
- `preview-safety.ts` supplies the connected library's raw-HTML policy. Source bytes remain unchanged.
- `rotation.ts` saves exact key updates before revocation. Retry resumes the saved plan after reload or a lost acknowledgement.
- `recovery-archive.ts` supplies authenticated binary chunks for large encrypted backups.
- `db.commitReplica` atomically saves projection, replica state, outbox, and received progress in existing stores.
- `db.enrollConnected` saves the complete initial selection after registration. It rejects intervening local edits and retains existing vectors and timestamps.

Initial registration stays separate from the local library until this transaction completes.
Other tabs discover complete records, rather than a partially seeded folder.
New scopes already have fresh activity leases, so renewal waits until those leases need extension.
Sessions retain the latest 64 acknowledgement digests. Another tab can acknowledge the same ciphertext without an outbox mismatch.
Unknown operations and different digests still fail acknowledgement checks.

## Reload request budget

The transport checks durable received progress before GET. Own ACK commits also advance the local author's stream cursor.
Skipping an indexed key never advances progress. New payloads still pass grant, signature, key-epoch, decryption, and atomic-storage checks.
Clients scan every index page, so delayed lower-sequence writes remain discoverable. Missing or older cursors cause a verified download rather than an unsafe skip.
Unknown document streams wait for their folder manifest instead of fetching ciphertext without a local session.

Folder polls request `directory: true` on the first index page. The responder includes the existing authenticated directory result in that response.
Document-only grants cannot request this metadata. The 100-item directory stays inside the existing KV and heap budgets.
The standalone directory endpoint and old index response remain available for older clients.

Cached-manifest reconciliation includes the current directory and exclusions in its version key.
Late registrations, deletions, and failed incoming downloads therefore remain recoverable without another manifest GET.
An unchanged hierarchy skips redundant tree writes and broadcasts. Native heap/budget and late-registration cases cover these paths.

## Incoming folder transactions

`materialization.ts` selects complete, registered parent chains in parent-first order. Invalid chains never default to the collection root.
Incoming sessions use a private repository during download, decryption, metadata updates, and key refresh.
No staged session changes the visible tree, shared replica storage, or upload-driver session list.
Initial pairing also stages the root and scope metadata until the hierarchy is ready.
After code approval, the target shows a native progress bar and a persistent download status.
The bar starts indeterminate, then counts completed documents. The tree still publishes only after the entire hierarchy is ready.
Cancel, Close, and page navigation abort the incoming requests. Failure and cancellation close the download dialog without publishing incomplete replicas.

`db.materializeConnected` resolves remote identities inside one shared write transaction.
Concurrent tabs reuse the winning local IDs, including folder IDs for child placement.
Existing bodies, pending uploads, timestamps, and local display preferences stay unchanged.
A changed root checkpoint rejects the prepared hierarchy. Retry reads the latest manifest.
Body commits retain the latest stored placement, so an asynchronous body update cannot undo a tree move.
`status.ts` keeps transport activity in memory per collection/member. Only ended state survives a stored tree reload.
Library refreshes use the current driver state instead of saved working/current/offline labels. Pending review markers remain unchanged.
Local non-UUID source identities survive replica metadata updates. Sync uses a separate portable UUID without publishing a local file or URL location.

Startup repairs duplicate rows only for equal collection/member/document identities.
The repair archives the complete affected scope before it deletes or detaches any duplicate row.
Distinct local text, review history, and pending projections stay under **Recovered Sync copies** without an active connection.
Archive failure aborts the entire repair. Encrypted backups retain the raw archive and restore one active connection per scope.
Same names and origins do not authorize a merge across different remote identities.

Focused browser coverage observes committed trees and visible row depths during initial pairing and concurrent downloads.
The concurrency case holds two stale tabs at the same request, forces download failure, and checks retry, exact text, reviews, and reload.
The repair case checks encrypted backup restoration of distinct text, pending projections, and raw pending packets.

Initial setup failures carry a fixed error code and setup step to a persistent error screen.
Debug traces use an allowlist and stay in tab memory, with no document names, content, identifiers, URLs, credentials, or raw error text.
An orphan-replica preflight failure retains the old record and creates no new collection. Diagnosis does not reset storage or discard pending work.
See [developer flags and diagnostics](../../README.md#developer-flags-and-diagnostics) for the generic enable, export, and disable actions.
Sync dialogs have no diagnostic or repair-backup controls. Preserved repair state is included only in full-folder encrypted connection backups.

The tests cover duplicate/reordered body updates, offline outboxes, local undo, malformed envelopes, forged authors, scope escape, storage failures, and database rollback.
The sandbox test runs the compiled P-256 verifier without browser globals or native Web Crypto.
The KV test double enforces the current 200-operation and 1 MiB value limits.
`sync-native.test.ts` also runs the assembled script through the real `secutils::js_runtime::JsRuntime`.
Its runner uses a 10 MiB heap and a 10-second deadline. The KV stub keeps stored bytes outside V8.
Write, exact retry, read, and renewal checks passed at the protocol limit. These checks do not use the production database.

The native check uses `MARS_NATIVE_RUNNER` and `MARS_NATIVE_DIR`.
The runner accepts three paths: the script, the JSON context, and the binary request body.
It returns the script's string result on stdout. A runtime error produces a nonzero exit code.

## KV rules

Each writer stores full encrypted CRDT checkpoints under immutable, newest-first sequence keys.
A newer checkpoint contains that writer's earlier operations. It is not a whole-text last-writer-wins replacement.
The responder stores ciphertext separately from its small signed header. The index exposes the header only after the ciphertext survives a read-back check.
This format avoids a second base64 layer around large ciphertexts. Both records have the same bounded lifetime.
Acknowledgement hashes bind the signed fields and the ciphertext hash through `checkpointReceipt`.
Polls rescan the key index. A timestamp cursor cannot safely track concurrent KV writes.

Quota slots use immutable generation ledgers. `ifAbsent` selects the winner for each generation.
Release appends a free generation. A delayed duplicate release cannot release a later claimant's quota.
Small releases happen after acknowledgement. Authenticated maintenance splits larger releases into bounded requests.

Quota ledgers, reservation references, and revocation fences need persistent metadata.
The default KV configuration permits this. A finite operator lifespan cap stops Sync with an explicit error.
Document checkpoints and activity leases still expire after 30 days.
Maintenance frees capacity after checkpoint expiry and clears abandoned reservations after a two-day grace period.

Activity renewal has two steps. Individual checkpoint requests renew the required records first.
The final document lease never outlives the earliest required checkpoint.
Ordinary polls do not renew activity.

## Short Live links

New Live URLs contain `#live=<secret>`. The secret is 16 random bytes, encoded as 22 base64url characters.
`live-link.ts` derives three independent SHA-256 values from `mars-live-v1:<purpose>:<secret>`:

- `id`: the first 16 bytes, formatted as a UUID with version and variant bits.
- `access`: the 32-byte bearer token sent to invitation endpoints.
- `encryption`: the AES-256-GCM key for the bootstrap and subsequent Live key updates.

The bootstrap encrypts the delegation signing key, owner public key, document key, epoch, bearer token, and wrapping key.
Authenticated data is canonical JSON `{purpose: "mars-live-bootstrap-v1", invitation: <id>}`.
The invitation stores only the encrypted bootstrap and token hash. Its signed policy bounds document, permission, and expiry.
The fragment secret and encryption key never enter requests. Knowledge of the bearer token cannot decrypt the bootstrap or key updates.
Expiry and revocation prevent bootstrap reads and joins. A revocation fence also blocks retries that attempt to restore the invitation.
Full Live links require an independent wrapping key. Links without it, or with a wrapping key equal to the bearer token, are rejected before requests.
Saved unsupported Live connections retain local documents and pending work but do not start transport.
Recovery applies the same requirement. Pending rotation packets must decrypt with their separate wrapping keys before any remote mutation.
Owner revocation remains available for unsupported links. They receive no new key-update packets.

Short Sync links use the same bounded bootstrap mechanism with the separate `mars-sync-v1` derivation domain.
The encrypted payload contains the invitation ID, access token, owner public key, and setup-wrapping key.
Its authenticated-data purpose is `mars-sync-bootstrap-v1`. Pair labels use the wrapping key, never the request token.
The invitation expires after ten minutes. Matching-code approval remains mandatory. Long Sync links also require an independent wrapping key.

Edit-access deletions carry the deleting actor's signature. Comment versions still require author ownership, including for editors.
Comment-only participants cannot delete another author's comment or a thread with known replies from other authors.
Concurrent replies to a deleted thread retain the existing recovery behavior.
`undo-delete` references the deleting actor's own deletion ID. It contains no replacement comment or thread text.
Projection cancels that deletion without cancelling other actors' deletions or edits. Seed comments retain their original bytes and authors.

Replies reference the latest observed operation from each author in the same thread.
Projection places causal predecessors before replies. Creation dates order independent and legacy comments, with operation IDs as deterministic tie-breakers.
Seed comments retain their stored order. Edits and deletion Undo retain each comment's original position.
Opening a saved connection rebuilds an outdated projection without changing signed history, pending packets, or network progress.

## Live-only teardown

Explicit enrollment and Sync-link creation mark a scope as personal Sync. Live-link creation alone does not.
After the final Live link is disabled, a standalone Live-only document enters a durable cleanup job.
`purge-live-document` installs an irreversible deletion fence, deletes encrypted checkpoints and bootstraps, and releases quota reservations.
Each request stays within the KV operation budget. Retry resumes cleanup after lost responses or navigation.
Requests stop on hidden tabs and pagehide. Cleanup resumes when the page becomes visible.
Cleanup refuses to delete a document that still has personal-device grants or a pending pairing invitation.
Small deletion and revocation fences remain so that retries cannot revive old access. They contain no document content or encryption keys.
Late writes cannot publish after deletion. Maintenance can reclaim their reservations without the abandoned-write grace period.
Recipients retain their saved projection and outbox. Their persistent badge shows a disconnected local copy.

## Activity and UI boundaries

Sync, source Live links, and recipient Live document panels have separate controls over the shared replication engine.
Recipients forward active links through Copy link → Live. The operation confirms invitation access and returns the original URL without creating a grant.
Expired or revoked links cannot be forwarded. Legacy saved recipients without the original URL retain the full-link fallback.
The Live document panel contains access and local-copy actions, not a separate Copy original link button.
View/comment recipients have no body-editing handles or mutable task controls. Their access badge remains visible in the toolbar.
Opening an enrolled folder renews its complete subtree, including folders without their own replica session.
`touch-folder` requires full-folder rights and a renewed root lease. Its lease cannot outlive that root.
Already-expanded folders also renew when their page becomes visible. Renewal aborts on hidden tabs and pagehide.

## Telemetry contract

### Operator-only test scopes

The existing responder also accepts `POST /sync/testing` with the operator credential from `MARS_STATS_API_KEY`.
It can open, list, or close exact test scopes. Scope creation records the run/test identity and a capability hash.
Closing a scope requires its exact scope, run, and test IDs. User records cannot qualify as test scopes.

Test requests carry `x-mars-test-scope` and `x-mars-test-token`, separate from participant authorization.
The responder maps every KV operation, including snapshot operations and quota ledgers, into `testing/v1/data/<scope>/`.
Returned keys and cursors retain their logical names. Invalid, expired, or closed capabilities cannot fall back to user storage.
Ordinary requests retain their existing namespace, limits, and wire formats.

The scope capability expires after one hour. Cleanup marks the scope as closing before deleting bounded batches of 64 entries.
An in-flight scoped write checks the scope after storage and removes its write if teardown already started.
Completed audit metadata expires after 14 days. Interrupted cleanup remains recoverable through operator metadata and private test manifests.
The registry exposes no operator credential, capability, document content, or participant credential.
See [the test cleanup commands](../../README.md#isolated-browser-test-data).

The operator-only endpoint uses the existing `MARS_STATS_API_KEY` secret.
It returns reserved capacity, the reservation unit, the maximum capacity, and the number of collections with reservations.
It returns no document names, content, credentials, or participant identifiers.
Reserved capacity includes prepared writes. It is not a measurement of actual stored bytes or active readers.
The existing `mars-stats` storage totals remain the source for actual stored bytes.
The dashboard shows sync allocation separately from actual stored bytes.
Scheduled samples also run bounded checkpoint cleanup. Missing sync telemetry does not replace storage measurements with zero.

## Release checks

The affected all-browser regression passed 492 cases with 14 expected skips.
Six additional marker checks passed in Chromium, Firefox, and WebKit, including the unchanged Markdown Preview surface.
Unit, type, lint, native-limit, snapshot-responder, and artifact-assembly checks passed.
The marketing-page check remains disabled because `SECUTILS_TOOLS_PROMO_HOME_INDEX` is unset. No promotion metadata changed.

Chromium checks passed for interrupted key distribution, live-key updates, and epoch activation after the service committed but its acknowledgement was lost.
Each check retained local edits and another authorized live link after reload and Retry.
Large encrypted recovery also passed a browser restore with an existing private library.

## Recovery formats

Small packages retain the version 1 JSON format and its 16 MiB file limit.
Larger packages use `.mars-recovery` archives. They support at most 512 MiB of serialized replica data.
Both formats include credentials, pending work, owned link secrets, revocation history, and any unfinished rotation.
Optional Download/Export all recovery uses these formats inside a ZIP, under `MARS connections/`.
The version 3 encrypted payload groups selected connection packages and local documents.
Legacy single-connection payloads remain readable. Restore validates the entire selection before one persistence transaction.
Partial folder selections recover only their selected documents. They do not resume enrollment of new documents outside that selection.
The storage snapshot captures the selection and its metadata in one read transaction.
Restore writes only after the whole package passes decryption and structural checks.

Version 2 archives contain:

1. The eight-byte `MARSREC2` marker.
2. A little-endian 32-bit header length.
3. A JSON header with `v`, `kind`, `salt`, `nonce`, `chunkBytes`, `bytes`, and `chunks`.
4. Sequential AES-GCM ciphertext chunks, each with its 16-byte authentication tag.

The normal chunk size is 4 MiB. The final chunk can be smaller.
PBKDF2-SHA256 uses 600,000 iterations and a random 16-byte salt.
Each nonce contains an eight-byte random prefix and a big-endian 32-bit chunk index.
Authenticated data binds the header, the chunk index, and the format purpose.
Missing, changed, or reordered chunks fail before any library write.

Close the previous client before restoring its credentials in another profile.

Pairing, offline work, draft recovery, unlink, deletion, failed storage, encrypted recovery, and library promotion passed focused engine checks.
Populated libraries passed both choices: separate copies, or a confirmed match with the original local ID and an independent backup.
The linked-file check used a real Chromium file handle. Remote edits did not write the file before explicit Save.
Native Firefox 159.0.1 passed pairing, bidirectional edits, reload, and private-library retention in two fresh profiles.

Live verification follows deployment. Follow `dev/plans/mars-sync-and-live-links.md` for the approved boundary.
