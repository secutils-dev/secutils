# Shared design system

Read this guide before changes to `dev/tools/shared/`, MARS chrome, or a migration of another tool.
The shared foundation reproduces the lightweight EUI look of MARS with CSS and native HTML.
It includes Borealis neutrals and EUI glyphs, without EUI components or React.
MARS and ECHO consume this foundation. Other tools keep their existing styles until their migration.

## Ownership

| Surface | Source |
|---|---|
| Neutrals, semantic colors, fonts, radii, shadows | `../shared/tokens.css` |
| Header, buttons, fields, panels, groups, menus, dialogs, toast, footer | `../shared/components.css` |
| EUI glyphs, SVG renderer, icon hydration | `../shared/icons.js` |
| ECHO app icon, isolated from its OG tile | `../echo/assets/echo-app-icon.svg` |
| Source-to-responder asset inclusion | `../shared-assets.ts` |
| MARS coral identity and document colors | `../js/mars/src/ui/tokens.css` |
| MARS sidebar and document layout | `../js/mars/src/ui/components.css` |

The CSS and glyph catalog are the source of truth for visual values.
Use the CSS variables instead of copies of color values.
Keep app layout, data state, and event handlers in the app.
Keep EUI glyph attribution in `icons.js` when moving or adding glyphs.

## Migrate a tool

1. Record its controls, theme behavior, dialogs, responsive layout, and state contract.
   The inventory is complete when every visible control has a corresponding shared primitive or app-specific rule.
2. Add these links before the app stylesheet:

   ```html
   <link rel="stylesheet" href="./shared/tokens.css" data-su-style="tokens">
   <link rel="stylesheet" href="./shared/components.css" data-su-style="components">
   ```

   `deploy.ts` replaces these links with inline styles before minification.
   MARS imports the same CSS through its bundle.
3. Use shared classes for every primitive in the inventory.
   Delete the corresponding local CSS rules.
   Keep the remaining CSS limited to app layout and app-specific content.
4. Replace UI glyphs with `<span data-icon="copy"></span>` and other catalog names.
   Import the renderer in the app module:

   ```html
   <script type="module">
   import { icon, hydrateIcons } from './shared/icons.js';
   hydrateIcons();
   // After creating dynamic markup: hydrateIcons(container).
   // For a dynamic button: button.innerHTML = icon('cross').
   </script>
   ```

   Deployment replaces this module path with an embedded data URL.
   Source pages work through an HTTP server that serves `dev/tools/`.
   Deployed pages need no shared-file requests.
5. Preserve the URL state format, responder script, metadata, and Skill link contract.
   Keep the footer and related-tools banner on tools-host pages.
   Use [components.md](components.md) for native dialog and popover behavior.
6. Run the checks in [../AGENTS.md](../AGENTS.md) and the verification section below.
   Completion requires every inventoried control to pass desktop, phone, theme, and keyboard checks.

## Primitive contracts

- Use `.su-header`, `.su-header-section`, and `.su-header-btn` for the 48px header.
- Use `.su-app-brand` for the app icon beside a stacked app name and publisher.
- Use `.su-app-icon` for a square 32px icon area, with app-specific artwork.
- Use `.su-app-brand-copy`, `.su-app-name`, and `.su-app-publisher` for the text stack.
- Keep both text lines visible on phones, with `Secutils.dev` under the short app name.
- Use `.btn` for a 32px button, `.btn-sm` for 28px, and `.btn-xs` for 24px.
- Use `.btn-primary`, `.btn-empty`, and `.btn-danger` for action emphasis.
- Use `.icon-btn` for a 28px icon button with an accessible label.
- Use `.input`, `.input-mono`, and `.textarea` for native fields.
- Use `.field-label` with an associated field or section heading.
- Use `.su-panel` for a content surface and `.su-dialog` for a native modal.
- Use `.toast` with a live region for action feedback.
- Keep visible keyboard focus and delayed `:user-invalid` feedback.
- Let native `color-scheme` follow `data-theme` for browser controls.
- Keep control heights fixed when controls share a row.
- Preserve labels on mobile unless an accessible name remains on the control.

The shared foundation owns component geometry and neutral colors.
An app identity can override `--brand*` and `--focus` in both themes.
MARS keeps its coral identity. ECHO uses the default Secutils yellow/maroon identity.
Use contrast-safe `--brand-text` for links in both themes.
Use `--brand-on` for text on filled primary buttons.

## Add an EUI glyph

1. Read the upstream SVG under `components/secutils-webui/node_modules/@elastic/eui/lib/components/icon/svgs/`.
2. Copy its inner markup into `ICONS` in `../shared/icons.js`.
   Preserve the path data and explicit fills of multicolor logos.
3. If the upstream viewBox uses a 32px grid, add its name to `LOGOS_32`.
   The renderer uses a 16px grid for other glyphs.
4. Run the glyph tests and inspect the glyph in both themes.

Use the EUI glyph catalog for every UI action.
App logo artwork remains app-specific.

For a file-backed app icon, use this source markup:

```html
<img class="su-app-icon" src="./echo/assets/echo-app-icon.svg" data-su-app-icon="echo" width="32" height="32" alt="">
```

The marker names `<app>/assets/<app>-app-icon.svg` under `dev/tools/`.
Deployment embeds the SVG as a data URL, so the served header needs no asset request.
Keep the decorative icon label empty when the adjacent text identifies the app.
Keep shared OG and header artwork aligned through an icon parity test.

## Verification

Run these commands from the repository root:

```bash
node --test scripts/tools-shared.test.ts
make tools-verify ARGS="dev/tools/echo.html dev/tools/mars.html"
make tools-check
make e2e-standalone-test ARGS="tools_shared_ui.spec.ts --browser=all"
```

For shared CSS or glyph changes, also run the MARS checks in [mars.md](mars.md#verify-a-change).
Inspect both themes at desktop and phone widths.
Make sure that native dialogs close and return focus correctly.
Make sure that fields, header rows, dynamic glyphs, and copy feedback remain usable.
Inspect the generated page for unresolved shared-file references.
Report any blocked check as incomplete verification.
