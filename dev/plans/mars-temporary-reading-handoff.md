# MARS temporary reading: verification record

Date: 2026-11-07
Status: native browser checks passed. The user skipped the remaining VoiceOver checks. Final deployment and regression results follow.

## Current outcome

- Initial implementation commit, before the final rebase: `09d70e06 feat(mars): add temporary reading and storage fallback`.
- The initial deployment succeeded for the MARS page, skill, and discovery aggregates.
- Initial deployed bundle SHA-256: `a1014666a6b8b7890a36a49f3c0b23217fabac5ac8cb0aca0e07ddc6fcd80c8d`.
- Final local bundle SHA-256: `0ffb97edefd0a804d08c2ad1089a87a4b77ec0ee6420a2f33d33cc1819b2ec35`.
- The resumed request authorizes fixes, commits, deployment, and a push.
- Markdown Preview retains its behavior. Its shared block-editor fallback receives the same accessible name as MARS.
- Legacy-link migration and unrelated plans remain unchanged.

## Implemented behavior

- `?temporary=1` and fragment `t=1` select an in-memory session before IndexedDB bootstrap.
- The New menu creates a temporary document in Source. Document options can open the current document temporarily.
- Storage denial and write-quota failures retain the document in temporary storage instead of losing the edit.
- Temporary sessions do not change the persistent library or its last-document metadata.
- The header and banner identify the session as temporary, rather than Saved.
- Only the current document returns from its compressed URL after reload. Other temporary documents and folders require export.
- Automatic URL recovery stops at 16 million source characters or a two-million-character fragment.
- Failed or oversized URL recovery displays a download warning instead of retaining a stale recovery link.
- Save current to library uses the portable URL. Larger documents require Markdown download and normal library import.
- Opening a file temporarily does not link it for autosave. Automatic file writes stay disabled in temporary mode.
- Temporary short-link deletion tokens stay in memory. Full and short shares retain the temporary-mode hint.
- Source editors have accessible names. Download anchors remain connected inside the active dialog until navigation completes.
- The plain-text block-editor fallback now has the name **Edit Markdown block**, like its CodeMirror counterpart.
- Authenticated `afterprint` messages restore focus after a requested print operation.
  Closed, replaced, and inactive format workspaces do not receive this focus change.

## Export policy

The UI, package README, and published skill document the export boundary.
Document-supplied `script`, `style`, `iframe`, `object`, `embed`, `base`, `meta`, and `link` elements are removed.
Event handlers, `javascript:` URLs, and internal source-range attributes are removed too.
Frontmatter and review metadata stay out of rendered exports. Markdown downloads retain them.
Ordinary HTML and inline styling remain. MARS adds its own reader and supported widget runtimes afterward.
The cleanup is not a complete HTML sanitizer. Controlled third-party embeds remain outside the current contract.

## Completed verification

- MARS unit tests: 77 passed. Type checking, E2E lint, `tools-verify`, `tools-check`, and diff checks passed.
- Temporary-mode browser cases: 18 passed across Chromium, Firefox, and WebKit.
- Full local regression: 354 passed, 12 expected GPU/touch-driver skips, and six selector failures.
  The failures were the new menu's accessible name and a previous assertion banning all `details` elements.
  Both selectors were corrected. All six affected cases then passed across the three engines.
- Storage-denial tests also deny BroadcastChannel access. All three engines passed after that final guard.
- Post-deployment live Chromium smoke: three passed for URL recovery, storage-denial export, and library isolation/promotion.
- The first resumed regression passed 359 cases, with 12 expected skips and one premature WebKit promotion assertion.
  The assertion now waits for the edited content. The promotion case passed five consecutive WebKit runs.
- A clean resumed regression passed 360 cases, with 12 expected skips.
- The fallback-editor case passed across Chromium, Firefox, and WebKit after its selector correction.
- The print-completion focus case passed across all three engines.
- The changed Markdown Preview fallback passed against its local source across all three engines.
- The final full regression after the native focus fix remains pending.
- `sh .husky/pre-push` passed, including Web UI tests, SQLx schema verification, nightly Rust formatting, Clippy, and 1,173 Rust tests.
- Promo-site synchronization remains skipped by the default `tools-check` command. The sibling site's known missing links are unrelated.

## Native browser results

### Safari 27.2 (21625.2.8.18.4)

The normal Safari application, not only the WebKit test driver, opened the native print dialog and saved a PDF.
PDFKit found one A4 page (595 x 842 points), readable document text, and no MARS application chrome.
The normal Safari Markdown download also matched the complete source byte-for-byte.
Safari requested download permission for localhost, which was allowed for this verification.

The normal Safari application also passed these checks three consecutive times after the final focus fix:

- `Cmd+S` in Source downloaded the complete Markdown source byte-for-byte.
- Escape closed the export dialog and restored focus to **Export document**.
- Cancel closed the native print dialog and retained the export workspace.
- Focus returned to **Print / Save PDF** when Safari was active again.
- The export workspace closed and reopened successfully after cancellation.

The native helper activates the owned Safari window before its focus assertion.
This excludes unrelated application switching from the page-focus result.
Safari native PDF evidence covers A4 portrait. The engine regression covers all four paper/orientation combinations.
The download helper was hardened during verification. The original no-file observation also involved Safari's download permission prompt.
Do not claim that the original failure had only one established cause.

### Edge 157.0.4341.42

Edge was installed with the user's permission through `brew install --cask microsoft-edge`.
An isolated normal Edge profile completed the actual print/save workflow.
The helper selected **Save as PDF**, never the physical printer action.
PDFKit verified one page, the document text, no application chrome, and these dimensions:

| Settings | Dimensions (points) |
| --- | --- |
| A4 portrait | 594.96 x 841.92 |
| A4 landscape | 841.92 x 594.96 |
| Letter portrait | 612 x 792 |
| Letter landscape | 792 x 612 |

The actual File System Access picker saved Markdown that matched the source byte-for-byte.
Cancel retained the source. Later edits in the temporary session did not automatically change the saved file.
The owned Edge process closed after every attempt.

### Assistive technology

The user enabled Accessibility access and the separate VoiceOver AppleScript permission.
VoiceOver ran briefly and produced actual navigation speech for temporary status and the named source editor.
The user then requested that the remaining VoiceOver checks be skipped because they were too slow.
VoiceOver stopped. Full spoken navigation, announcements, editing, export-tab, modal-focus, and dismissal coverage remains unverified.
Engine tests and accessibility-tree inspection do not replace those checks.

## Final publication checks

1. Complete the final local regression after the native focus fix.
2. Deploy MARS and the shared Markdown Preview fallback label.
3. Compare the deployed artifacts with the local build.
4. Complete the required production regression.
5. Record the results, rebase unpublished commits onto the current remote branch, and push.
6. Stop the owned preview server.

Use the three-engine suites from the repository root:

```bash
MARS_BASE_URL=http://localhost:7373 make e2e-tools-test ARGS="mars.spec.ts mars-export.spec.ts mars-import.spec.ts mars-models.spec.ts mars-tables.spec.ts mars-temporary.spec.ts --workers=3"
```

## Native automation notes

- `safaridriver --mcp` exposes the installed Safari engine, but automation sessions suppress native print dialogs.
  Use a normal Safari window for native print/save verification.
- Each MCP client can leave a separate Safari `--automation` process after tab closure. Track and close only owned processes.
- System Events returned an empty `entire contents` list for Safari despite accessible child elements.
  Direct Accessibility traversal was substantially faster and more reliable than recursive AppleScript traversal.
- The active keyboard layout is not Latin. `keystroke` produced garbled path text during one native save.
  Use physical key codes and layout-independent value/paste operations. Preserve the user's clipboard if used.
- The incorrectly named test PDF was moved from Applications to the approved artifact directory. No unrelated files were removed.
- Edge's Fluent print-preview buttons required native keyboard activation after Accessibility focus.
  An Accessibility press reported success without opening the native Save dialog.
- The final Safari focus helper uses a zero-delay task after `afterprint`, not an animation frame.
  Safari can pause animation frames while another application is active.
- Each clean local regression started with a fresh in-memory preview fixture.
  Production tests retain the short-link limit of 20 creations per hour.

Native helpers and artifacts remain under the approved OpenCode temporary directory:

```text
/private/var/folders/9j/8dppfsyj4j70c0hfldz8l2g80000gn/T/opencode/
  mars-native-ax
  mars-native-ax.swift
  mars-native-ui.applescript
  mars-safari-client.mjs
  mars-native-artifacts.json
  mars-native-1794028470013/safari-native-print.pdf
  mars-native-1794028470013/safari-native-markdown.md
  mars-safari-1794052295621/results.json
  mars-safari-1794052295621/safari-keyboard-save.md
  mars-edge-1794051863147/results.json
  mars-edge-1794051863147/a4-portrait.pdf
  mars-edge-1794051863147/a4-landscape.pdf
  mars-edge-1794051863147/letter-portrait.pdf
  mars-edge-1794051863147/letter-landscape.pdf
  mars-edge-1794051863147/Edge native completion.md
```

The owned Safari verification windows and Edge processes are closed. VoiceOver is off.
Final publication checks still use one owned preview server on port 7373.
Unrelated browser windows and Docker services remain unchanged.
Preserve the unrelated untracked files `dev/plans/markdown-viewer-colored-models.md` and `dev/plans/responder-kv-counters.md`.
