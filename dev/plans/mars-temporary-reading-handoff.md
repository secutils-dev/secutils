# MARS temporary reading: paused handoff

Date: 2026-11-07
Status: implementation committed and deployed. Native verification is incomplete and paused at the user's request.

## Current outcome

- Implementation commit: `09d70e06 feat(mars): add temporary reading and storage fallback`.
- Deployment: `make deploy-tools ARGS=mars` succeeded for the MARS page, skill, and discovery aggregates.
- Deployed bundle matches the local build. SHA-256: `a1014666a6b8b7890a36a49f3c0b23217fabac5ac8cb0aca0e07ddc6fcd80c8d`.
- The current request authorizes commit and deployment, not another push. The implementation commit remains local.
- Markdown Preview, legacy-link migration, and unrelated plans remain unchanged.

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
- A clean full regression after all final changes remains pending.
- Promo-site synchronization remains skipped by the default `tools-check` command. The sibling site's known missing links are unrelated.

## Native browser results

### Safari 27.2 (21625.2.8.18.4)

The normal Safari application, not only the WebKit test driver, opened the native print dialog and saved a PDF.
PDFKit found one A4 page (595 x 842 points), readable document text, and no MARS application chrome.
The normal Safari Markdown download also matched the complete source byte-for-byte.
Safari requested download permission for localhost, which was allowed for this verification.

The keyboard Save shortcut, cancellation/focus-return flows, and broader native format combinations remain pending.
The download helper was hardened during verification. The original no-file observation also involved Safari's download permission prompt.
Do not claim that the original failure had only one established cause.

### Edge 157.0.4341.42

Edge was installed with the user's permission through `brew install --cask microsoft-edge`.
A headed Edge session prepared PDF pages and invoked the MARS print action.
That session closed before the native preview/save workflow completed. Native PDF save and File System Access save remain unverified.

### Assistive technology

The user enabled Accessibility access. Actual VoiceOver interaction has not run.
Accessible names were added to the source editor and the CodeMirror block editor.
Inspect the plain-text block-editor fallback during the remaining accessibility checks.

## Resume next

1. Start one local preview server with `npm --prefix dev/tools/js/mars run serve`.
2. Complete headed Edge native Save as PDF and file-save workflows. Inspect the resulting files.
3. Run real VoiceOver with normal Safari through source editing, temporary status, export tabs, modal focus, and dismissal.
4. Complete Safari keyboard-save and cancellation/focus-return checks.
5. Run a clean local regression and the required post-deployment regression.
6. Record actual results without substituting engine tests or accessibility-tree inspection for native/VoiceOver checks.
7. Commit any verification fixes and redeploy only if application artifacts change.

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
```

The owned Safari verification window (ID 54592) is closed, and the preview server on port 7373 is stopped.
No test shell remains running. Unrelated browser windows and Docker services remain unchanged.
Preserve the unrelated untracked files `dev/plans/markdown-viewer-colored-models.md` and `dev/plans/responder-kv-counters.md`.
