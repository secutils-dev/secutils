// Imported first by main.ts: styles and icons must exist before other modules run.
//
// Owns the one app stylesheet. esbuild loads each `.css` file as text. This module joins the
// files in cascade order: shared tokens, MARS brand tokens, document styles, shared components,
// MARS components, export styles. `hydrateIcons()` then fills each empty `[data-icon]` element
// of the shell. The body stays hidden (`html.mars-boot`) until main.ts has loaded the tree.
import tokens from '../../shared/themes/mars.css';
import sharedTokens from '../../shared/tokens.css';
import sharedComponents from '../../shared/components.css';
import docCss from './doc/doc.css';
import components from './ui/components.css';
import exportCss from './export/export.css';
import { hydrateIcons } from './ui/ui.ts';

const style = document.createElement('style');
style.textContent = `${sharedTokens}\n${tokens}\n${docCss}\n${sharedComponents}\n${components}\n${exportCss}`;
document.head.append(style);
hydrateIcons();
