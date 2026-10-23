// Imported first by main.ts: styles and icons must exist before other modules run.
import tokens from './ui/tokens.css';
import docCss from './doc/doc.css';
import components from './ui/components.css';
import { hydrateIcons } from './ui/ui.ts';

const style = document.createElement('style');
style.textContent = `${tokens}\n${docCss}\n${components}`;
document.head.append(style);
hydrateIcons();
