# MARS: cross-browser sync and live links

Date: 2026-11-07
Status: responder-only implementation approved on 2026-11-07. Release checks passed and deployment was approved on 2026-11-08.

Implementation checkpoint: the responder protocol, enrollment, pairing, folder manifests, live sessions, key rotation, durable outbox, and preview policy are implemented locally.
The normal MARS entry now integrates the connected viewer. Local preview and deployment share the same responder assembly.
Draft recovery, unlink/delete behavior, encrypted recovery, and `mars-stats` allocation metrics are present. Release and operational checks remain in progress.
No deployment is authorized until the implemented behavior and remaining verification gaps receive operator review.
On 2026-11-08, focused checks passed for populated-library matching, atomic recovery, unlink, deletion, same-profile edits, and explicit file Save.
The native responder passed write, exact retry, read, and renewal checks at the protocol limit within the 10 MiB heap.
Two fresh native Firefox profiles passed pairing, bidirectional edits, reload, and private-library retention.
Interrupted key rotation now retains a durable plan with exact key envelopes. Chromium checks passed after lost service acknowledgements and reload.
Large encrypted recovery now uses authenticated binary chunks. Its browser restore retained oversized pending work and an existing private library.
The final affected all-browser regression passed 492 cases with 14 expected skips. Six additional marker checks passed in all engines.
Artifact review passed. The user authorized deployment after these checks.
See `dev/tools/js/mars/src/sync/README.md` for the current contracts and remaining release gates.

### Follow-up: Sync dialog and table selection

The initial Sync release and this follow-up are deployed. The user approved the follow-up deployment on 2026-11-08:

- **Sync** contains setup, invitations, connection removal, backups, and local disconnection.
- Setup opens the invitation screen. Back returns from subpages. The close icon closes the complete flow.
- **Password-protected backup** appears under **More** and explains the information that it restores.
- **Stop syncing here** retains the local document. Other connected browsers continue to sync.
- **Make independent** separates non-sync sources. The document menu has no separate **Invite** entry.
- Preview formatting uses the selected table cell's source range and restores the selection after each format.

The eight failures in the follow-up regression passed after dialog cleanup and test isolation fixes.
Desktop and touch cell-formatting checks passed in Firefox, Chromium, and WebKit, including Undo/Redo and exports.
The full affected regression passed 410 cases with 12 expected skips.
The final build passed 15 focused Sync and cell-formatting cases across all three engines, including header cells and cross-cell selection guards.
The unit suite passed 144 cases with one opt-in native skip. Typecheck, lint, artifact review, tool parsing, and standalone responder checks passed.
Markdown Preview selection checks passed 34 cases. It remains a separate tool with no redirect or retirement.
Live Firefox passed pairing, edits in both directions, reload, and private-library retention against the deployed initial release.
Private stats returned current Sync totals and rejected unauthenticated access.
The marketing-page check remains unavailable because `SECUTILS_TOOLS_PROMO_HOME_INDEX` is unset. No promotion metadata changed.

`make deploy-tools ARGS="mars"` published MARS, its skill, and four discovery aggregates.
The live application matches the verified bundle, SHA-256 `43b9aff4d5cae68dada1775ec4bf6a95d334d46f66f2d965fe23e0fcc71cbcd4`.
The live skill matches its source. COOP remains `same-origin`, and COEP remains `credentialless`.
Private stats returned current Sync totals after deployment, with request tracking disabled.
The targeted live checks passed 14 cases and skipped three local-only cases.
Live cell formatting passed in all three engines. Chromium passed the complete Sync flow, and Chromium/Firefox passed pairing and named-browser removal.
Four remaining Sync cases reached the five-new-collections-per-hour limit from the verification connection.
A direct live check confirmed HTTP 507 with `membership-or-document-limit` on collection creation. The local document remained intact.
The general live MARS regression passed all 128 cases. Markdown Preview was not redeployed.

### Follow-up: creation limit and imported folders

The user approved 50 new collections per IP per hour and redeployment on 2026-11-08.
`LIMITS.collections` is the single source for enforcement and the advertised limit. Storage and folder-item limits remain unchanged.
Creation throttling now returns HTTP 429 with `collection-creation-limit`, separate from item and membership limits.
The Sync dialog explains this error and retains the local document.

Local inspection of the supplied ZIP found 24 documents and seven subfolders, or 32 items including the selected root.
Registration succeeded, but an empty document viewer blocked folder uploads. The publication guard now applies only to the open document's own connection.
After this fix, the supplied ZIP reached Up to date with 32 registered items and 49 acknowledged updates.
The archive contents stayed on the local in-memory responder. External requests were blocked during this check.

The unit suite passed 146 cases with one opt-in skip. Typecheck, lint, tool parsing, and 20 standalone responder checks passed.
The native heap check passed after its test operations used separate quota probe ranges instead of random overlapping ranges.
The all-engine Sync regression passed 81 cases with two expected skips. One local-storage recovery case exposed a missing Retry action.
The dialog now reports unsaved local changes instead of Up to date and provides Try again. All 12 focused final-build checks passed across Chromium, Firefox, and WebKit.

`make deploy-tools ARGS="mars"` published the 50/hour limit, error wording, and folder-upload fix.
The live `/sync/limits` endpoint reports 50 collections, 100 selected items, ten members, and the unchanged 100 MiB collection quota.
The deployed app matches the verified build, SHA-256 `e86be655aafd5073ab60bdb1e084ba4f1168b66911197b2e2c515b76f66ef674`.
All nine focused live checks passed across Chromium, Firefox, and WebKit, including the 32-item imported folder, pairing, and edits from the paired browser.
The general live regression passed all 128 cases. The private ZIP was not uploaded to production.

### Follow-up: retain search indexes during Sync

The user reported that enabling Sync started indexing the existing folder again.
The connection repository set `updatedAt` on every replica commit, including enrollment metadata, checkpoint preparation, and acknowledgements.
Smart search uses `updatedAt` to detect changed document content. It treated these protocol-only writes as document edits.
Existing vectors remained stored, but their timestamps no longer matched.

The local fix retains `updatedAt` when the document's text and name stay unchanged. The index format and model settings remain unchanged.
The regression reproduced three unnecessary embedding calls after Sync on the old build.
All seven focused checks passed after the fix, including all three engines, another tab, text edits, renames, and indexing pause/resume.
The supplied ZIP also passed: all 24 documents retained their cached indexes, with zero additional embedding calls after Sync.
An initial archive check returned before the first index pass finished. The corrected check waits for the completed cache before measuring Sync.
Archive checks used the local responder and a mock model worker, with external requests blocked. The fix is included in the footer deployment below.

### Follow-up: Sync terminology and footer

The user approved these changes and deployment on 2026-11-08:

- Back appears at the bottom-left of each subpage. The close icon closes the complete flow.
- **Sync link** replaces the browser-pairing **Invite** label and its user-facing copy.
- New links use `#sync=`. Existing `#invite=` links still open until they expire. Signed payloads and responder routes remain unchanged.
- **Stop sync** is a footer button. Confirmation explains that this browser keeps an independent copy and other browsers continue syncing.
- **Browsers & links** and **Backup** are compact main-screen controls. The More section is removed.
- This deployment also includes the search-index retention fix.

The unit suite passed 147 cases with one opt-in native skip. Typecheck, lint, tool parsing, and artifact review passed.
The all-engine Sync regression passed 100 cases with two expected skips. The checks cover new and legacy pairing links, cached search indexes, backup restoration, offline work, revocation, and table formatting.
Desktop and phone visual review passed. Footer alignment and page overflow checks passed at 320px and 760px in both themes across all three engines.

`make deploy-tools ARGS="mars"` published the new flow, its skill, and four discovery aggregates.
The live app matches the verified build, SHA-256 `31128dec3d7709f28f13eb4d6e8cb63aab86cd6097140d45111051db9c70dfa8`.
The live skill matches its source. The 50/hour limit and isolation headers remain unchanged.
All 30 focused live checks passed across Chromium, Firefox, and WebKit. They include new and legacy Sync links, footer layout, backup navigation, and retained search indexes.
The general live regression passed all 128 cases. Together with the 30 focused live checks, all 158 post-deployment browser cases passed. Markdown Preview was not redeployed.

### Follow-up: EUI button and feedback patterns

The user approved these changes and deployment on 2026-11-08:

- Loading actions use an EUI-style spinner in the disabled action button.
- Sync link copy explains the one-time approval and the shared review identity. The original window is not needed for later synchronization.
- Copy feedback is a small Copied tooltip, with no added paragraph or dialog movement.
- Both confirmation screens use the same aligned, read-only code display.
- Primary, secondary, empty, and flush button variants follow EUI guidance across MARS. The design-system guide documents the patterns.
- Backup is explained as optional recovery for Sync access and pending upload state, distinct from ZIP or Markdown content exports.
- Browser rows omit the redundant subtitle. Disconnect and disable actions use EUI's link_slash glyph with accessible labels and tooltips.
- Browser and Live link groups use whitespace rather than a trailing row divider.
- Live links use spaced native radio controls, a labelled expiry field, and plain-language access and saved-copy explanations.

Typecheck, 148 unit cases, seven shared-foundation checks, and 66 all-engine shared UI checks passed.
The 42 focused all-engine Sync/Live checks passed. The broader regression passed 514 cases with 14 expected skips.
The two WebKit failures passed in a focused retry. Status-only notifications no longer postpone indexing a real document change.
All 30 final UI checks passed across the three engines, including narrow-screen Share navigation. Three real-model browser checks passed.
All four shipped model/weight combinations passed the quality gate. The final unit suite passed 149 cases with one opt-in native skip.
Final desktop and phone visual review passed. The style guide records EUI button hierarchy, flush alignment, spinner, icon, and clipboard feedback rules.

`make deploy-tools ARGS="mars"` published the EUI-pattern update, its skill, and four discovery aggregates.
The live app matches the verified bundle, SHA-256 `b42b6991ce2a7dd1f909d3ea15da0d7f5904b0a23e61ed305e64d13c9f823fcd`.
The live skill matches its source. The 50/hour creation limit and isolation headers remain unchanged.
All 15 focused live UI checks and all 128 general live MARS checks passed. The final deployment passed 143 browser cases.
Markdown Preview and the other tool pages were not redeployed.

### Follow-up: short Live links and library promotion

The next user request covers tooltip sizing, device terminology, Live link names, plain-language library saving, index retention, lifetimes, shared-default spacing, and comment deletion.

- Tooltip width is intrinsic and viewport-capped. Native popovers retain visible arrows. Focus keeps the tooltip open after pointer departure.
- The origin and recipient connection UIs use devices. Existing saved names remain unchanged. Generated Live names use an ASCII hyphen.
- Library saving explains retained documents, unsent changes, and continued access while the link remains active.
- Storage promotion transfers the selected vector record. The index cache loads the persistent library after a backend change.
- New Live links use a 22-character fragment secret and encrypted KV bootstrap. Access and encryption keys use separate derivation domains.
- Live expiry choices are 1, 7, 14, and 30 days, with 7 days selected by default. Existing long links remain supported.
- Set shared default has a 12px top margin.
- Edit-access deletion remains signed by the deleting actor. Comment-only participants cannot delete others' comments or threads with known replies from others.

Unit, type, shared-foundation, 126 standalone, three real-model browser, and four model-quality checks passed.
All-engine checks passed for index retention, permission boundaries, short-link privacy, long-link reception, and device disconnection.
Desktop and phone visual review passed. All 36 final UI cases passed, including focused retries after fixture corrections.
The 152 unit cases passed with one opt-in native skip. The large-checkpoint fixture now permits exact retries for bounded quota-slot collisions.
The wider regression passed 522 cases with 14 expected skips. Its three fixture failures passed in the final focused UI runs.
All 525 applicable cases now have passing results.
The marketing-page check remains unavailable because its path is not configured.

Deployment published MARS, its skill, and four discovery aggregates. The live app matches the verified build.
App SHA-256: `a2f060781a00af7ed939e57eda9f2c9533c42bb047c8671d99044304259a1dc2`.
The live skill matches its source. Isolation headers and the 50/hour creation limit remain unchanged.
Focused production checks now include library-index preservation and Live edit/comment permission boundaries.
All 24 focused production cases passed in Chromium, Firefox, and WebKit.
All 128 general production cases passed. The deployment passed 152 live browser cases in total.
The owned local preview process is stopped. Markdown Preview and other tool pages remain unchanged by this follow-up deployment.

### Follow-up: moderation Undo, Live teardown, and export recovery

The user approved these changes after reporting moderation Undo failures and ambiguous connection controls.

- `undo-delete` cancels only the deleting actor's signed deletion. It restores original comments and threads without changing authorship.
- Connections replaces the enrolled-item Sync label. Disconnect this device retains a local copy; Stop sharing disables every Live link for the document.
- Live-only enrollment remains distinct from personal Sync. Disabling its final link starts durable, bounded remote cleanup.
- Cleanup deletes encrypted checkpoints, bootstraps, and key records, then releases reserved capacity. Small irreversible fences remain without content or keys.
- Cleanup pauses on hidden tabs and pagehide, resumes when visible, and survives lost responses. Late writes and stale quota references remain fenced.
- Ended recipients show an unlink badge and a visible local-copy notice. Temporary copies warn that reload cannot restore an expired link.
- Download and Export all retain normal output by default. Active connections add an optional password-protected recovery choice.
- The optional ZIP contains readable documents and one encrypted connection archive. Import can restore connections and local documents together.
- Partial recovery reads and exports only selected replicas. Existing library documents remain unchanged after restoration or a wrong password.
- Recovery supports legacy single-connection files and the existing large binary format. Advanced Markdown output retains its immutable capture.

The 157 unit cases passed with one opt-in native skip. Type, lint, source parsing, and 60 snapshot-responder checks passed.
All 12 new flow cases passed in Chromium, Firefox, and WebKit. The wider regression passed 528 cases with 14 expected skips.
Its nine stale status-text assertions passed after correction. All 537 applicable regression cases have passing results.
Desktop and phone visual review passed. Phone connection actions now wrap instead of clipping.
All six final narrow-footer checks passed. The approved deployment published MARS, its skill, and the discovery aggregates.
The live app and skill match the verified sources. App SHA-256: `fa71c580b98312c543107cdb0a4268f5c130c1e8d9ec5034e81f14f12217e768`.
Isolation headers and the 50/hour creation limit remain unchanged. The owned local preview is stopped.
All 12 focused production flows and all 128 general live cases passed. The deployment passed 140 production browser checks.
No Markdown Preview or other tool page deployment occurred.
Marketing-page verification remains unavailable because its path is not configured.

### Follow-up: separate Sync and Live sharing

The user selected separate dialogs for personal Sync, source Live links, and recipient Live documents.
The user approved deployment on 2026-11-08. This follow-up is deployed.

- **Sync** manages personal devices on the source and paired devices.
- **Live links** manages links for one source document. **Live document** shows recipient access, expiry, and local-copy actions.
- **Keep as local copy** stops updates to that recipient without disabling the original link.
- View/comment recipients have no body-editing handles or mutable tasks. A document-specific badge shows their access.
- The Share and document/folder menus have one **Copy link** entry. Its dialog selects the sharing mode, format, access, and expiry.
- Full snapshots never expire. Short snapshots offer 1, 7, or 14 days. Live links offer 1, 7, 14, or 30 days.
- Folder links remain snapshots. Document and folder download entries both use **Download**.
- Short Sync links contain a 22-character fragment secret and expire after ten minutes. Matching-code approval remains required.
- Legacy long Sync and Live links remain supported. Live key derivation remains unchanged.
- Folder activity renews nested folder metadata and unopened documents. Visible, expanded folders also renew without another navigation.
- Privacy explains encryption. The Sync dialog explains inactivity expiry and retained local copies.

The unit suite passed 160 cases with one opt-in native skip. Typecheck, tool parsing, and 60 snapshot-responder checks passed.
Desktop and phone artifact review passed after the captures awaited rendered content and completed theme changes.
The dark preview uses the correct colors. The earlier pale-text capture preceded the completed theme change.

The broad regression passed 554 cases with 14 expected skips. Its 16 failures passed after focused corrections.
Clipboard fixtures now track the dialog's click handler instead of the removed menu actions.
Local snapshot tests use separate test clients to avoid the unchanged network quota.
The final Live-only teardown check no longer expects a closed dialog's Close button.
The recipient panel now reloads its current document reference after library saving.
Temporary-document tests select the current tree item instead of a persistent-library anchor.

All 576 applicable browser cases have passing results across the broad run and focused corrections.
This total includes six new cases for recipient independence and source disconnection across the three engines.
The final six recovery checks passed in Chromium, Firefox, and WebKit.
These checks cover library saving, original-link forwarding, independent copies, and interrupted cleanup with a retained Retry action.
The final assembled HTML passed link creation, recipient permissions, and separate Sync setup in all three engines.
`make deploy-tools ARGS="mars"` published MARS, its skill, and four discovery aggregates.
The live app and skill exactly match the release artifacts. The app has 383,885 bytes.
App SHA-256: `5390b17bd2bfc70656ae4d970035b052b594ae160d3e9b12685d19e6cd4a9b38`.
Markdown negotiation and the discovery skill digest match. Isolation headers and quotas remain unchanged.
The live limits report the ten-minute Sync invitation expiry. The owned local preview is stopped.
All 128 general production cases passed. Sharing, permissions, recipient independence, and source disconnection passed in all three engines.
Legacy pairing and long Live reception passed in all three engines. Three local-only pairing cases were skipped in production.
Final-link cleanup passed in all three engines after the Firefox assertion allowed time for the service operation.
The Chromium imported-folder retry passed after the test waited for complete materialization before opening a nested folder.
All 155 applicable production cases passed. Three local-only cases were skipped.
These corrections change tests only. No deployed code change or additional deployment was required.
No Markdown Preview or other tool page deployment occurred.
Marketing-page verification remains unavailable because `SECUTILS_TOOLS_PROMO_HOME_INDEX` is unset. VoiceOver remains user-skipped.

### Follow-up: Copy link defaults and simpler controls

The user requested this refinement after the separate-dialog deployment and approved deployment on 2026-11-08. This batch is deployed.

- Copy link defaults to **Snapshot + Full**. Full appears before Short.
- Short snapshots and all Live links offer 1, 7, 14, or 30 days. The choices omit calendar dates.
- Full snapshots still never expire. Folder links remain snapshots.
- Before creation, the result area explains the **Copy link** action instead of showing an empty input.
- After creation, the read-only URL replaces that instruction. Clipboard failures retain the URL for manual copying.
- The result area reserves its height. Changing choices clears the stale URL and Copied tooltip.
- Source **Live links** management appears at the bottom-left of Copy link, not in the item or Share menu.
- Returning to an existing Live-links dialog does not create a duplicate modal.

The shared snapshot responder accepts 30 days. Its Markdown Preview copy remains identical, with unchanged Markdown Preview lifetime controls.
The unit suite passed 160 cases with one opt-in native skip. Typecheck, lint, tool parsing, and 60 snapshot-responder checks passed.
Visual review passed at 320px and 1280px in both themes. Copy feedback and management actions did not move the dialog.
The focused regression passed 51 cases. Three older tests assumed Short was the default and now explicitly select it.
All six final checks passed in Chromium, Firefox, and WebKit. All 54 applicable cases have passing results.
The owned local preview is stopped. `make deploy-tools ARGS="mars"` published MARS, its skill, and four discovery aggregates.

The live app and skill exactly match the release artifacts. App SHA-256: `bc2f345f0f9737dec0c0a784909f05c244d84101f2991cb57ad8cccf80c01333`.
Markdown negotiation and the discovery skill digest match. Isolation headers and Sync limits remain unchanged.
All 15 focused production checks for the new controls passed in Chromium, Firefox, and WebKit.
Cleanup passed in all three engines after the Firefox retry waited for completed link removal and foregrounded recipients before status checks.
The general production regression passed 126 cases. Both Chromium retries passed.
One reload hit the unchanged ten-request responder limit. The other case timed out during a gutter geometry read.
The retry used one worker without concurrent suites. These corrections changed tests only.
All 146 applicable production checks passed. No additional deployment was required.
Markdown Preview and other tool pages were not redeployed.
Marketing-page verification remains unavailable. VoiceOver remains user-skipped.

### Follow-up: alternating comment order

The user reported A:1, B:2, A:3, B:4 appearing as 1, 2, 4, 3 after each reply synchronized.
The user requested a fix, a regression test, and deployment. This batch is deployed.

The deterministic regression reproduced the exact incorrect order before the fix.
Projection sorted random operation IDs, and reply dependencies included only the replying author's previous operation.

- Replies now reference each author's latest observed operation in the same thread.
- Projection puts causal predecessors first. Original creation dates order independent and legacy comments, with deterministic ID tie-breakers.
- Edits retain the original creation time and comment position. Seed comments retain their stored order through edits and deletion Undo.
- Resolution and deletion operations do not reorder earlier legacy comments.
- Opening an outdated saved projection repairs its order without changing signed history, pending packets, sequence numbers, or network progress.
- The signed schema, author checks, encrypted packets, and existing link formats remain unchanged.

Three new unit regressions cover the alternating sequence, delivery permutations, equal timestamps, clock skew, legacy projections, edits, and Undo.
The browser regression uses A/B/A/B and deliberately orders random IDs so that comment 4 sorts before comment 3 on the old implementation.
It waits for every received comment before the next reply, then checks editing, reload, and the actual Markdown download.
All 163 unit cases passed with one opt-in native skip. Typecheck, lint, tool parsing, and the local 24-case review regression passed.
All six final browser checks passed. The comment-order screenshot shows 1, 2, 3, 4 with A, B, A, B authors.

`make deploy-tools ARGS="mars"` published MARS, its skill, and four discovery aggregates.
The live app and skill exactly match the release artifacts. Isolation headers remain unchanged.
App SHA-256: `fcfa68bc18d0a0d0daaabf11db33c9515b88f728390096855b652a7d66d4c9ad`.
The production ordering regression passed in Chromium, Firefox, and WebKit, including every synced reply, editing, reload, and Markdown download.
All 128 general production cases passed without concurrent suites. This deployment passed 131 production cases in total.
The owned local preview is stopped. Markdown Preview and other tool pages were not redeployed.
Marketing-page verification remains unavailable. VoiceOver remains user-skipped.

### Follow-up: opening retained empty threads

The supplied Live link opened with `thread-owner-required`. Inspection used only review metadata and did not edit the shared document.
The source retained an empty legacy thread after its comments were deleted.
An already-ready editor notified persistence during document hydration, before the exact source and saved baseline were installed.
The viewer filtered the empty thread, so this incomplete state became an unintended deletion intent. Comment-only access correctly rejected it.

Host document opening now suppresses edit persistence until the source and saved baseline are ready.
Connected parsing also retains empty threads, so comment-only readers can reply without generating a deletion of hidden metadata.
The ownership checks remain unchanged. Read-only opening must not publish body or review packets for view, comment, or edit access.
The new regression forces the editor to initialize before receiving the link. It reproduced the failure before the fix.
It also covers reload, library saving, zero append requests during opening, retained metadata in a Markdown download, and a permitted new reply.

The unit suite passed 163 cases with one opt-in native skip. Typecheck and lint passed.
All 15 initial browser checks passed. All nine retained-thread cases have passing results after the focused WebKit retry.
The fixture reloads the source after enrollment so its seeded comment controls reflect the connection's access.
The complete correction is deployed. The original supplied link shows Can comment, Up to date, and an available Reply field without an error.
The supplied document was not edited. Synthetic fixtures verify permitted replies.
The live app matches the final release artifact, SHA-256 `d119fa95cd66a1168a7054ce325a58c57e8a9d75da48ab8cd8667641d8eab543`.
The synthetic production regression passed in Chromium, Firefox, and WebKit, including the permitted new reply.
All 128 general production cases passed without concurrent suites. The final correction passed 131 production cases in total.
The owned local preview is stopped. No Markdown Preview or other tool page deployment occurred.

### Follow-up: unified recipient forwarding

The user requested removal of Copy original link from Live document and approved deployment.
Active recipients now use Copy link → Live to forward the owner's original URL.
The dialog shows fixed access and expiry. It hides format, access, and lifetime controls during forwarding.
The operation confirms invitation access without creating a grant. Legacy saved records retain the full-link fallback.
Expired or revoked links cannot be forwarded. An open dialog clears stale results and disables forwarding when the link ends.
Snapshot sharing remains available. The dialog does not silently switch a selected Live action to Snapshot.
The ended Live document panel contains access and local-copy actions, with no separate copy button.

All 163 unit cases and 27 focused browser cases passed. Typecheck, lint, tool parsing, and desktop/phone visual review passed.
The forwarding regression covers Short/Full links with view/comment/edit access, unchanged URLs, unchanged grant counts, and revocation during an open dialog.
Deployment published MARS, its skill, and four discovery aggregates. The live app and skill exactly match the release artifacts.
App SHA-256: `9ed5e53838bdea63833d03739c9edc0831275aa203abf327957eb67ae3afee08`.
Markdown negotiation and the discovery skill digest match. The owned local preview is stopped.
Production forwarding checks passed in Chromium, Firefox, and WebKit. They verified all six Short/Full and view/comment/edit combinations without new grants.
All 128 general production cases passed without concurrent suites. This deployment passed 131 production cases in total.
Marketing-page verification remains unavailable. VoiceOver remains user-skipped.

### Final polish and Git publication

On 2026-11-09, the user selected “Share a copy or keep others up to date.” for the Share menu's Copy link help.
The item retains its short accessible name and exposes the help as its description.
Privacy and Credits now have a 960px desktop cap, with the shared phone and height limits unchanged.
The documents-only download note has a 16px paragraph break. Privacy correctly distinguishes snapshots and lists the 30-day snapshot choice.

All six focused UI/download checks have passing results after the responsive-sidebar fixture waited for the breakpoint update.
Visual review passed. MARS typecheck, 163 unit cases, and 21 shared/stats script cases passed.
The full pre-push gate passed: 258 Web UI cases, SQLx, nightly formatting, Clippy, and 1,173 API cases.
Upstream's performance-history-only advance was fast-forwarded without changing the working files.
The staged publication contains the accumulated verified MARS implementation, shared fixes, documentation, and tests.
The unrelated colored-model and responder-counter plans remain untracked and untouched.

Deployment published MARS, its skill, and four discovery aggregates. The live bytes match the final release artifacts.
App SHA-256: `4d83c08f24c8e33e7dede8afad809e59f793025bb8b2590a3db616ece95c5c3d`.
All six focused and 128 general production checks passed. Git publication is paused for the table-resizing correction below.
Marketing-page verification remains unavailable. VoiceOver remains user-skipped.

### Correction before Git publication: connected table resizing

The supplied recording showed widths changing during drag and snapping back after release.
The local reproduction used the supplied Markdown against the local responder only. Its contents were not uploaded to production.
The safety sanitizer removed the inert `su-table` comment. The Markdown retained the new proportions, but the next render reverted to automatic layout.

The renderer now passes only validated numeric table metadata through sanitization, using a renderer-generated carrier.
Raw HTML still loses its data attributes and styles. Widths are applied after sanitization in both Preview and HTML/PDF output.
The source Markdown format, source ranges, and editing permission checks remain unchanged.
The new regression covers enrollment, pointer resizing, Undo/Redo, keyboard sizing, reload, snapshots, and HTML/PDF proportions.
It also rejects forged carriers from raw HTML. The pure metadata test rejects styles, scripts, invalid values, and extra fields.
The six focused resize/export cases passed in Chromium, Firefox, and WebKit. The six sanitizer-boundary cases also passed.
The initial Chromium PDF failure came from the local test header, which reached CDNs and triggered rejected CORS preflights.
The table fixture now sends that header only to the local MARS server. No export implementation change was necessary.
The unit suite passed 164 cases with one opt-in native skip. Typecheck, lint, tool parsing, and the configured tool checks passed.
The exact minified release passed resize and reload checks in all three engines. Preview and PDF visual review passed.
The full table run passed 126 cases with two expected touch skips. One new Chromium case reloaded before its keyboard edit finished saving.
The fixture now waits for the exact keyboard width in IndexedDB before reload, then checks Up to date.
All five Chromium stability runs and all three final fixture cases passed. All 127 applicable table cases now have passing results.

`make deploy-tools ARGS="mars"` published the correction, its skill, and four discovery aggregates.
The live app and skill match the reviewed release bytes. Deployment uses gzip level 9 without changing the verified bundle bytes.
App SHA-256: `a3ef818873bcc900c424078705d6e064b28266c9db463ccbb6d8edf094b092e7` (385,813 bytes).
The skill digest and Markdown negotiation match. COOP remains `same-origin`, and COEP remains `credentialless`.
All 12 focused production cases passed across Chromium, Firefox, and WebKit. Production used synthetic content, not the supplied Markdown.
All 128 general production cases passed. This correction passed 140 production browser cases in total.
The Git publication includes the accumulated MARS implementation, refinements, and this correction. The unrelated plans remain untracked and untouched.
The owned local preview is stopped. Marketing-page verification remains unavailable, and VoiceOver remains user-skipped.

### Production test cleanup (2026-11-09)

The user authorized removal of all unused Sync/Live collections, while retaining snapshot short links.
The operator API cleared only `sync/v1/`: 9,035 entries were removed.
Storage decreased from 6,431,151 bytes and 9,352 entries to 184,166 bytes and 317 entries.
Sync reservations decreased from 58,720,256 bytes across 139 collections to zero.
The snapshot namespaces and historical telemetry were not cleared.

The user selected authenticated test namespaces and approved redeployment of the existing responder.
The new `/sync/testing` operation records run/test identity and issues a separate, one-hour scoped capability.
All test KV data, including snapshots and quota ledgers, stays under `testing/v1/data/<scope>/`.
Teardown removes only that scope. It rejects mismatched ownership and prevents late scoped writes from restoring deleted data.
Closed audit metadata expires after 14 days. Interrupted scopes retain metadata for exact recovery.
Private manifests retain scoped capabilities and any ordinary preservation-canary delete tokens. They never contain the operator credential.
The cleanup CLI lists ownership metadata and resumes exact manifests or expired test scopes.

The local unit suite passed 170 cases with one opt-in native skip. Native heap and invocation-limit checks passed separately.
The eight sandbox/isolation checks, nine shared/client checks, and 20 snapshot-responder checks passed.
All three engines passed pairing, connected table resize/reload/export, and ordinary-short-link preservation during scoped teardown.
The broad browser run passed 443 cases with four expected skips. Both fixture failures passed after focused corrections.
The temporary fixture no longer sends its old isolation header to CDNs. The deletion fixture waits for connection readiness after reload.
All 445 applicable browser cases now have passing results. All nine final focused cases passed in the three engines.
Deployment published the updated responder, unchanged application and skill, and discovery aggregates.
The application bytes retain SHA-256 `a3ef818873bcc900c424078705d6e064b28266c9db463ccbb6d8edf094b092e7`.
The deployed responder matches the verified compiled script. Request tracking remains disabled, and the isolation headers remain unchanged.
The production run passed 282 cases with 56 expected local-only skips. Its three canary cases used non-canonical fixture bytes.
Production correctly rejected those bytes. The fixture and local KV stub now enforce canonical base64, matching the native backend.
All three corrected preservation cases passed. All 285 applicable production cases have passing results.
The final registry audit found 249 test scopes, all closed, with 2,327 test entries removed automatically.
Ordinary Sync collections and reservations remain zero. Current storage is 297,934 bytes across 568 entries, including the expiring test audit records.
The owned preview is stopped. Marketing-page verification remains unavailable, and VoiceOver remains user-skipped.

## Approved implementation boundary

The user approved implementation with the existing MARS responder and its KV store.
Do not add Secutils Rust routes, PostgreSQL migrations, account requirements, or another service.
The earlier dedicated-service proposal is superseded by the decisions in this section.

- Use **Sync** for enrollment and **Sync link** to connect another browser. Use **Share → Live** to share with another person.
- Keep the local library as the default. Only selected content enters remote storage.
- Use encrypted Yjs body checkpoints, with immutable per-writer sequence keys. Do not use whole-text last-writer-wins replacement.
- Keep signed review operations separate from body state. Enforce view/comment/edit permission in the responder and received projections.
- Use bounded, short polling requests. Hidden/offline tabs abort reads and resume when visible/online.
- Treat KV pagination as key pagination, not as a reliable chronological change cursor. Concurrent or delayed writes must not disappear behind a timestamp cursor.
- A newer writer checkpoint includes that writer's previous acknowledged and pending body operations. Store and read back the new checkpoint before pruning older checkpoints.
- Renew the latest required checkpoints before renewing a document's 30-day activity lease. Polling alone does not renew this lease.
- Opening a document renews that document. Opening a synced folder renews its selected subtree.
- Expiry ends remote access. Existing local copies and unpublished edits remain recoverable. Re-enrollment is explicit.
- Keep snapshot storage and its existing limits separate from sync namespaces and creation throttles.
- Reserve capacity for checkpoints and cleanup. The responder's 1 GiB and entry limit cover all anonymous collections, not each user.
- Initially bound collections, members, grants, document counts, update size, and storage. Return the effective limits before enrollment.
- Add aggregate sync usage to `mars-stats`, without names, content, invitations, credentials, or participant identifiers.
- Remove **Move to top level** and **Export…** from document menus. Rename **Download .md** to **Download**. Keep the document-view export workspace.

The protocol spike must prove concurrency, retry, expiry, permissions, and malicious-content isolation before public release.
If these guarantees require Secutils changes, stop and prepare a new plan instead of crossing the approved boundary.
After implementation and checks, explain the final behavior in simple English and wait for deployment approval.

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
| Storage | Existing MARS responder KV, with a versioned protocol and bounded immutable checkpoints. No new Secutils service. |
| Text concurrency | Yjs `Y.Text` for document-body operations, behind a narrow replication adapter |
| Review concurrency | Signed, typed review operations with stable thread/comment IDs, separate from body updates |
| Ownership | Account-free collection creation with scoped client credentials and anonymous abuse controls. |
| Guest access | Later live viewers/reviewers join through a scoped link. No account is required. |
| Encryption | Per-document data keys, supported Web Crypto primitives, authenticated envelopes, and signed membership grants |
| Local persistence | Existing IndexedDB stores, with namespaced replica, credential, cursor, and outbox records |
| Source compatibility | Ordinary Markdown and existing `su-comments` / `su-table` formats remain portable |
| Disconnection | **Stop sync** in the Sync footer retains a local copy. **Make independent** separates non-sync sources. |

No account is required. Collection quotas, creation throttles, and 30-day inactivity expiry limit anonymous storage use.

The first release excludes presence cursors, chat, public folder publishing, remote binary attachments, and automatic filesystem write-back.
It does not replace current snapshot links or retire Markdown Preview.

## User flows

### Connect clients

1. Select a document or folder and choose **Sync**.
2. Show exactly which documents and comments will enter encrypted remote storage.
3. Create the collection and upload its initial state after confirmation.
4. Open **Sync link** and copy its one-time pairing link.
5. Open the link in the recipient client and give it a name, for example **Work Firefox** or **Laptop**.
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
Provide accessible labels such as **Synced** and **Live document: can comment**.
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
The badge details provide client management and, later, live-link management. Keep global privacy/settings separate from document actions.

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

Disconnecting one client is not remote deletion, link revocation, or disconnection of every other client.
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
| Client member | Independent signing/encryption keys, permissions, and revocation state |
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

Extend the MARS responder under a separate `/sync/` namespace, with bounded payloads and explicit authorization.
Use the MARS origin and scoped bearer credentials. Do not require shared login cookies or an API account.

The responder KV supports `ifAbsent`, TTL storage, and bounded watch/list operations. It does not expose revision compare-and-swap or multi-record transactions.
Use immutable sequence records and read-back acknowledgements instead of mutable shared heads.
Use short polling rather than parked isolates. Keep the existing `?s=` contract and its per-IP short-link budget unchanged.

Define versioned operations for collection creation, pairing approval, authorized bootstrap, append/acknowledge, changes, checkpoints, and revocation.
Each update identifies its document, stream, operation ID, author, key epoch, and protocol version.
The responder deduplicates retries with immutable records and read-back checks. It rejects an operation ID reused with different bytes.
Checkpoint indexes are bound to the authorized scope. A live-document subscription never exposes sibling updates.

Start with bounded short polls per active connected scope, not a long-poll per document.
Do not park a responder isolate while it waits for changes.
Stop network waits when hidden/offline and resume from the acknowledged cursor when visible. Retain offline work locally.
Allow an already-started upload to finish when a tab becomes hidden. A bounded final prepared batch can attempt a keepalive upload on pagehide.
Keepalive delivery is best-effort and size-limited. Correctness must not depend on unload callbacks or background-sync APIs unavailable in Firefox.
Use backoff with jitter and explicit Retry after failures. Later WebSocket/presence transport uses the same update protocol.

Compact logs into authenticated encrypted checkpoints only after durable storage and read-back.
An old cursor triggers checkpoint bootstrap plus pending-operation replay, not loss of offline edits.
Membership/deletion checks apply during replay as well as during ordinary upload.

The service returns its limits. The UI shows usage before enrollment and explains refused uploads without truncating local documents.
Use the approved 30-day inactivity policy and bounded collection limits. Show the effective limits before enrollment.
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
Use the approved account-free creation and inactivity policy. Prove the KV protocol before implementing public collection creation.

### 1. Service and durable replica

Add the responder protocol, quotas, idempotent append/ack, authorized bootstrap, checkpoint indexes, cleanup, and revocation.
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

Run affected MARS/Markdown Preview suites and responder gates from their task guides. Root API changes are outside scope.
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
The sync protocol belongs in `src/sync/`, with a separately compiled responder entry and executable protocol tests.
Deployment assembles this entry with the existing MARS responder. No Rust feature module or SQL migration is required.
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
