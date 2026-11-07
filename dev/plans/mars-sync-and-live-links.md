# MARS: cross-browser sync and live links

Date: 2026-11-07
Status: design proposal. This document authorizes no backend deployment or library migration.

## Outcome

A user links personal and work Firefox profiles once. Selected documents and folders then synchronize without manual share-link exchange.
The same document identity, replication protocol, and permissions later support live links between different people.
Both features use encrypted remote storage. Neither requires a local daemon, a browser extension, or shared Firefox credentials.

The useful distinction is **one connected document, several local replicas**, rather than repeated imports of unrelated snapshots.
The current local-only library remains the default.

## Scope and decisions

| Decision | Proposed direction |
| --- | --- |
| Initial product | Opt-in synchronization of selected documents or one folder subtree between a user's browsers |
| Later product | Live document links with view, comment, or edit permission, using the same replication engine |
| Storage | Durable, typed Secutils API with PostgreSQL. Keep the MARS app deployment as a tool responder. |
| Text concurrency | Yjs `Y.Text` for document-body operations, behind a narrow replication adapter |
| Review concurrency | Signed, typed review operations with stable thread/comment IDs, separate from body updates |
| Ownership | Recommended initial policy: a Secutils account creates a collection and owns its storage quota. Paired browsers use scoped credentials. |
| Guest access | Later live viewers/reviewers join through a scoped link. They do not need the owner's account. |
| Encryption | Per-document data keys, supported Web Crypto primitives, authenticated envelopes, and signed membership grants |
| Local persistence | Existing IndexedDB stores, with namespaced replica, credential, cursor, and outbox records |
| Source compatibility | Ordinary Markdown and existing `su-comments` / `su-table` formats remain portable |
| Disconnection | Reuse **Unlink from source**, with source-specific behavior and a single confirmation dialog |

Creator authentication is a proposed product decision, not an existing MARS requirement.
It limits persistent-storage abuse without forcing a login in every profile. Account-free collection creation requires an explicit quota/retention policy before release.

The first release excludes presence cursors, chat, public folder publishing, remote binary attachments, and automatic filesystem write-back.
It does not replace current snapshot links or retire Markdown Preview.

## User flows

### Connect two browser profiles

1. Select a document or folder and choose **Sync across browsers…**.
2. Show exactly which documents and comments will enter encrypted remote storage.
3. Create the collection and upload its initial state after confirmation.
4. Choose **Link another browser** and copy a one-time pairing link.
5. Open the link in the second profile and name that browser, for example **Work Firefox**.
6. Confirm the matching pairing phrase in the first profile.
7. Create connected local replicas in the second library.

Completion: an edit or comment in either browser appears in the other without importing another link.
Other personal/work documents remain local. Pairing expires if either browser does not complete confirmation.
The original browser needs to be open during pairing, not during later synchronization.
For a populated second library, compare existing origin matches before connecting them. Offer an explicit merge or keep the copies separate.
Seed each remote document once. A joining browser must not concatenate its independent seed with the creator's content or discard unpublished edits.
Names and content hashes are matching hints, not remote document identities.

A synced folder includes its selected subtree and later documents created inside that subtree.
Moving existing local documents into it requires confirmation before their first upload.
Moving a synced document outside the subtree asks whether to disconnect it or move it within the collection.
Overlapping enrollment must not create two competing bindings for the same local document.

### Resume, offline edits, and failures

Local edits save before network transmission. A durable outbox survives tab closure and browser restart.
On reopening, the client obtains the current checkpoint and missing updates, then replays its pending operations idempotently.
Other profiles do not need to be open simultaneously.
Only acknowledged uploads are available to another profile while the source browser is closed.
Closing before **Up to date** can leave pending work in the source profile until it opens again.

Show **Saved locally · waiting to sync** during offline work. Show **Up to date** only after service acknowledgement.
This status does not mean that another person read the document.
Expired credentials, quota failures, and revoked access retain the local content and pending work.
Rejected work becomes a recoverable local branch instead of disappearing or recreating a deleted remote document.

### Live links, on the same foundation

1. In Share, select **Snapshot** or **Live**. Keep current snapshot behavior unchanged.
2. For Live, choose **Can view**, **Can comment**, or **Can edit**.
3. Show the audience, access lifetime, and encrypted-storage boundary before creating the link.
4. Open the live link as a temporary connected session by default.
5. Persist its replica and credentials only after **Save live document to library**.

Completion: two authorized browsers see body changes, replies, and resolution changes through the existing document UI.
A single-document link grants no access to its parent folder, siblings, or the owner's other devices.
Creating a live link for an existing synced document does not copy it into a second collaboration system.

View-only sessions disable mutations but retain permitted reading/export behavior.
Comment permission permits review operations, not body edits. Edit permission does not imply permission to grant access or delete the remote resource.
An offline edit rejected after revocation stays available as a local copy. Previously downloaded content cannot be remotely erased.

## Connected-document UI

### Indicators

Keep the normal document/folder icon. Add one small trailing connection badge beside its name, without moving the action button or rename field.

- Personal sync: circular-arrow glyph from the EUI catalog, matching the existing `refresh` style.
- Live shared document: existing chain-link glyph.
- Both: one connection badge, with its tooltip/popover describing both relationships. Do not stack two badges.
- Folder badge: summarize descendants. A queued or failed child prevents an **Up to date** folder summary.

Use existing tokens and glyph procedures, not Unicode symbols or a new icon library.
Provide accessible labels such as **Synced across your browsers** and **Live document: can comment**.
Hover/focus reveals details. A tap opens the same details on mobile.
Animation appears only during actual work and obeys reduced motion. Color alone does not communicate status.

| State | Visible description |
| --- | --- |
| Connected, acknowledged | Up to date |
| Pending local operations | Saved locally · waiting to sync |
| No network | Offline · changes saved locally |
| Rejected or ambiguous work | Needs attention |
| Read-only grant | Live · view only |
| Revoked or expired grant | Connection ended · local copy retained |

The header combines save and connection status. Avoid a second permanent toolbar or an always-running spinner.
The badge details provide browser management and, later, live-link management. Keep global privacy/settings separate from document actions.

### One disconnection action

Reuse the existing **Unlink from source** menu position and confirmation pattern for documents and connected folders.
Explain the actual result rather than adding separate **Stop sync**, **Leave collaboration**, and **Unshare** menu items.

| Connection | Result of unlinking |
| --- | --- |
| File or imported URL | Existing behavior: forget that source, retain text/comments, and reset the source identity |
| Private sync | Stop this replica's synchronization, retain its current local copy, and give that copy an independent identity |
| Live document | Leave this connection and retain a local snapshot; it no longer receives or publishes updates |
| Connected folder | Apply the chosen disconnection to its subtree, with the affected count shown first |
| Multiple connections | One dialog lists the connections and lets the user choose which to detach |

Disconnecting one browser is not remote deletion, link revocation, or disconnection of every other browser.
Owner-only revocation remains inside connection details. Ordinary Delete explains whether it removes a local copy or deletes from the collection.
Pending work must be retained locally before unlinking. Late requests cannot reconnect the replica or resume uploads.
For disconnection inside a synced folder, persist a per-replica subscription/enrollment exclusion.
Keep its copy local-only or move it outside the synced subtree.
The next manifest refresh must not re-enroll that copy or silently import a duplicate of the disconnected remote document.
Remote folder deletion affects connected bindings only. Retain local-only descendants and queued edits in a recoverable local location.

Introduce a typed connection model. Do not overload `originId` or make the existing `app.unlink()` clear every relationship indiscriminately.
A local file association and a remote document binding are distinct. Incoming remote edits must not silently write to a linked disk file.
For the first release, connected documents require explicit disk Save; remote-update autosave to disk is outside scope.

## Shared data model

Use separate local IDs and remote IDs. A remote document can have a different local tree node in each profile.

| Record | Responsibility |
| --- | --- |
| Collection | Owner, limits, authorized membership, and encrypted selected-folder manifest |
| Remote document | Stable ID, encrypted body state, review state, shared layout, and deletion state |
| Participant | Logical person. A user's paired profiles share this identity; another reviewer does not. |
| Browser member | Independent signing/encryption keys, permissions, and revocation state |
| Local binding | Local node → remote document/collection, source relationships, and effective permission |
| Outbox entry | Stable operation ID, local operation bytes, prepared envelope, and acknowledgement state |
| Checkpoint/cursor | Authorized replica state and progress through the service's update history |
| Live grant | Scoped document permission, expiry, parent grant, and key envelopes; no collection-wide secret |

Synchronize body/frontmatter, review threads, names, selected-folder structure/order, reading width, and saved diagram/model metadata.
Keep local file handles, disk paths, theme, current document, folder expansion, search vectors, export drafts, and snapshot deletion tokens local.
Synchronize the document's shared layout defaults, not a reader's local width override or transient diagram/model navigation.
Review acknowledgement belongs to a participant. One person's **Mark reviewed** does not acknowledge another person's review.
Connected thread resolution is shared according to permission. Current snapshot-import resolution semantics remain unchanged.

## Replication and editor integration

Use Yjs from the first connected release, not a whole-document last-writer-wins upload loop.
Personal sync and later live editing share the same body-operation format and document IDs.
Keep Yjs behind an adapter so transport, encryption, UI, and Markdown serialization do not depend on its internals.

The body CRDT retains source text, including frontmatter and `su-table` comments. Store structured reviews separately.
Concurrent rewrites of a trailing JSON review block must not produce interleaved, invalid Markdown comments.
Preserve stable imported thread/comment IDs. Give new review operations unique IDs and a verified author identity.
Concurrent edits of the same comment retain both versions for resolution. Never silently lose a reply.

The Markdown projection retains unchanged source bytes and existing review-block formatting until review content changes.
Changed reviews use the current serializer and escaping rules. Transport credentials never enter Markdown, snapshots, HTML, PDF, or agent handoffs.

Integrate at actual mutation boundaries:

- CodeMirror source edits use an incremental collaboration binding.
- Textarea fallback edits produce minimal source-range changes, not whole-document replacement.
- Preview formatting, table operations, block moves, and comment actions produce typed local operations.
- Remote changes update the editor incrementally without resetting selection, scroll, or undo history.
- IME composition retains its draft and applies deferred projections after composition ends.
- Block-editor drafts retain relative source anchors. A deleted/overlapping remote target becomes a visible recovery choice.
- Remote changes cancel unfinished table gestures before applying new source mappings.

Use CRDT-relative positions for connected source anchors, while retaining quote/context anchors in portable Markdown.
Local undo tracks this browser's edits. It must not undo incoming changes from another browser or participant.
Existing immutable export captures remain immutable; later remote changes offer **Refresh preview**.
Export and local Save do not wait indefinitely for a network acknowledgement.

CRDT convergence is not semantic correctness. Contradictory table edits or invalid Markdown remain visible and recoverable.
Deletion uses tombstones. Offline edits to a remotely deleted document create a recovered local branch, not automatic resurrection.
Validate concurrent folder moves for cycles and missing parents. Resolve invalid effective placement deterministically into a recoverable folder.

## Durable service and transport

Create a dedicated typed service under `/api/mars-sync`, with PostgreSQL transactions, bounded payloads, and explicit authorization.
Use the existing API origin and narrowly scoped CORS for MARS/local development, with bearer credentials rather than shared login cookies.

The current responder KV supports `ifAbsent`, TTL storage, and bounded watch/list operations. It does not expose revision compare-and-swap.
Its short-link TTL, quota, and parked-isolate model are unsuitable as the sole production sync service.
Reuse concepts and tests, not the existing `?s=` contract or its per-IP short-link budget.

Define versioned operations for collection creation, pairing approval, authorized bootstrap, append/acknowledge, changes, checkpoints, and revocation.
Each update identifies its document, stream, operation ID, author, key epoch, and protocol version.
The service deduplicates retries transactionally and rejects an operation ID reused with different bytes.
Server cursors are opaque and bound to the authorized scope. A live-document subscription never exposes sibling updates.

Start with one bounded long-poll per active connected scope, rather than one poll per document.
Use cheap asynchronous server waits, not a JavaScript isolate per waiting browser.
Stop network waits when hidden/offline and resume from the acknowledged cursor when visible. Retain offline work locally.
Allow an already-started upload to finish when a tab becomes hidden. A bounded final prepared batch can attempt a keepalive upload on pagehide.
Keepalive delivery is best-effort and size-limited. Correctness must not depend on unload callbacks or background-sync APIs unavailable in Firefox.
Use backoff with jitter and explicit Retry after failures. Later WebSocket/presence transport uses the same update protocol.

Compact logs into authenticated encrypted checkpoints only after durable commit.
An old cursor triggers checkpoint bootstrap plus pending-operation replay, not loss of offline edits.
Membership/deletion checks apply during replay as well as during ordinary upload.

The service returns its limits. The UI shows usage before enrollment and explains refused uploads without truncating local documents.
Document retention, history retention, member limits, storage quotas, and abuse controls require an operator policy before public release.
Remote expiry is a visible disconnection state, not permission to replace the local library with an empty one.

## Encryption, permissions, and pairing

Generate independent per-document data keys. Bind envelopes to document ID, stream, key epoch, and protocol version.
Use authenticated encryption with fresh nonces. Retries reuse the same persisted envelope, not the nonce with different plaintext.
Wrap document keys for authorized browser members. Sign membership grants and update envelopes.
Choose and review the concrete native Web Crypto envelope scheme during the protocol spike; do not invent an unaudited cryptographic protocol.

A reusable live link receives only its document's keys and capabilities. It never contains a collection master secret.
Reading/decryption rights and server write rights are different capabilities.
Keep keys and invitations out of queries, logs, analytics, error reports, and exported documents.
Invite material belongs in fragments; consume one-time pairing invitations and remove them from the active URL.
Fragments can remain in browser history. Explain that risk in pairing/live-link details.

**Comment-only access must be enforceable, even though the service cannot decrypt content.**
Body updates use an edit-authorized stream. Typed review operations use an author-bound review stream.
The server verifies scope, stream permission, identity, and signatures before storage.
Clients validate allowed operation schemas and signed membership before projection.
A commenter cannot smuggle a body update through an opaque, all-purpose CRDT blob or claim another author's identity.
Resolution, membership, and deletion commands have explicit permission checks. Hiding a toolbar is not authorization.

Pairing uses a short-lived, single-use invitation, fresh member keys, and a transcript-bound confirmation phrase.
The existing browser approves the requested scope before wrapping keys for the new browser.
Separate browser credentials allow independent revocation. Revoke affected grants and rotate keys before publishing later protected content.
Revocation prevents future access; it cannot remove plaintext or keys already copied by a recipient.

Offer a clearly labeled encrypted recovery package. Account login alone cannot recover client-only decryption keys.
The service can still observe opaque IDs, traffic sizes/times, membership, and storage ownership.
Encryption does not bypass company policies or protect against a compromised MARS application delivering malicious JavaScript.

Treat received live content as untrusted, even when another editor has permission to change the document.
Audit preview DOM sinks, raw HTML, SVG, math, diagrams, URL attributes, and internal source-range attributes before public live sharing.
Document content must not execute in the application origin or access another document's storage or credentials.
The current export cleanup is not a complete sanitizer and does not establish this live-preview boundary.
Use a reviewed sanitization/isolation policy and supporting CSP. Preserve source bytes while applying safety rules to the rendered view.

## Local storage and temporary mode

Keep database versions 1 and 2 readable. Do not downgrade, delete, or bulk-convert existing libraries.
Use namespaced records in the current stores unless a measured need justifies a new store and a reversible migration.
Add an explicit atomic persistence API for local projection, replica state, pending operations, and bindings.
Do not hold an IndexedDB transaction open across encryption or network awaits.
Persist a local operation before preparing/sending its envelope. Delete its outbox entry only after a matching acknowledgement.
Persist received state and its cursor together. A storage failure cannot advance a cursor past an unsaved update.

One profile's tabs coordinate through Web Locks/BroadcastChannel where available. Unique operation IDs also make accidental duplicate senders safe.
BroadcastChannel is a local coordinator, not cross-profile transport.
On storage failure, retain the current edit, pause automatic publication, and offer recovery/download before changing persistence mode.

Temporary live sessions retain replicas and credentials in memory only. Explicit review actions can publish permitted operations for that session.
Read-only opening does not persist credentials, enter the library, or publish mutations.
Keep a live route distinct from the compressed temporary snapshot route; URL recovery must not accidentally discard or expose live credentials.
Reload can recover acknowledged live content. Unsent temporary work requires a visible export warning, not a false recovery promise.
No OPFS fallback, filesystem handle, or background daemon is required for this feature.

## Implementation phases and gates

### 0. Protocol and editing spike

Pin/audit Yjs and prototype body operations, review projection, source anchors, permission-separated streams, and envelope recovery.
Compare bundle/runtime cost against the current local-only app. Load collaboration machinery only for connected sessions where practical.
Completion: two isolated replicas converge after reordered/duplicate/offline updates; unauthorized body updates fail; source/export byte tests pass.
Malicious-document fixtures must also prove that live content cannot execute in the app origin or access unrelated library data.
Resolve creator authentication and retention policy before implementing public collection creation.

### 1. Service and durable replica

Add typed routes, SQL migrations, quotas, idempotent append/ack, authorized bootstrap, cursors, compaction, and revocation.
Add replica/outbox persistence without changing existing library behavior.
Completion: crash/reconnect/replay tests preserve every local edit, and storage/auth/quota failures retain recoverable work.

### 2. Personal-profile sync

Add opt-in enrollment, browser pairing, document/folder badges, connection details, and unified unlink behavior.
Integrate incremental source/preview/review updates and participant-local undo/acknowledgement.
Completion: owned personal/work Firefox test profiles synchronize a selected folder after restart, including offline concurrent edits and comments.
An unselected personal document never reaches the service. Linked disk files remain unchanged after remote edits.

### 3. Live document links

Add scoped live grants and temporary view/comment/edit sessions to the same remote documents.
Exercise independent participants, author permissions, resolution changes, expiry, revocation, and local recovery branches.
Completion: a single-document guest cannot enumerate or decrypt sibling documents, and a commenter cannot write body updates through any API path.
Later live-folder grants and optional presence are separate extensions, not a second replication system.

### 4. Release and operational verification

Run affected MARS/Markdown Preview suites and root API gates from their task guides.
Add `mars-sync.spec.ts` plus isolated replica, crypto/envelope, authorization, and persistence tests.
Cover Chromium, Firefox, WebKit, owned real Firefox profiles, mobile connection UI, both themes, keyboard use, and reduced motion.
Completion requires passing changed-surface checks, reviewed generated artifacts, exact deployed bundle/skill checks, and live sync smoke tests after an authorized deployment.

Required failure cases:

- Transport: duplicate/reordered updates, lost acknowledgements, expired cursors, and tab crashes.
- Access: interrupted pairing, corrupted envelopes, and revoked writers with queued work.
- Isolation: executable document content, forged source-range attributes, sibling enumeration, and unauthorized stream mutations.
- Data: deleted documents, folder cycles, simultaneous comment edits, and clock skew.
- Storage: private/denied storage and quota exhaustion.
Also cover source-editor IME, textarea fallback, block drafts, table gestures, exports during remote edits, and same-profile duplicate tabs.
Keep current snapshot-link, temporary-reading, HTML/PDF privacy, and v1/v2 database rollback tests.

## Code boundaries and guides

Current integration points: `src/app.ts`, `src/store/db.ts`, `src/tree/model.ts`, `src/sidebar/sidebar.ts`, `src/doc/viewer.js`, and `src/doc/editor.js` within `dev/tools/js/mars/`.
Use a new `src/sync/` area for protocol, replica, transport, credentials, enrollment, and connection presentation.
The Rust service belongs in a dedicated feature module with handlers, persistence, and schema examples, not in the Markdown responder script.
Keep common renderer fixes consistent with Markdown Preview. Keep synchronization-specific hooks MARS-only.

Read `dev/tools/AGENTS.md`, its MARS/design-system/URL-state guides, and `dev/agent-guides/http-routes.md` before implementing their respective surfaces.
Use package scripts and task guides for commands rather than copying a second command catalog into this plan.

## Plan cleanup

The completed parity and temporary-reading plans are retired from `dev/plans`.
Current guides, source, and tests describe their implementation. Historical results remain in Git history.
The MARS guide retains native menu/split-view gaps, incomplete rich-print combinations, and the user-skipped VoiceOver checks.
Retirement is documentation cleanup, not authorization to retire Markdown Preview or implement legacy-link migration.

## UI prerequisites, separate from sync implementation

The current branch replaces the table action strip with selected-axis handles and compact popovers inside the reading pane.
Keyboard/touch reordering, redraw handling, undo, and proportional layout remain covered by regression tests.
The formatting toolbar now handles coarse-pointer cancellation and visual-viewport placement. The user confirmed that the formatting menu works again in Android Firefox.
Table cells also support isolated Markdown edits, and column menus provide left, center, and right alignment.
The requested PDF explanation hints are removed, and standalone theme glyphs are centered.

Preserve these selection/control lifecycles during live editor integration. Refer to their source and regression tests instead of implementing parallel controls.
