// Imported first by main.ts: styles and icons must exist before other modules run.
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
