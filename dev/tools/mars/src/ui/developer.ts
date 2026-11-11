import { featureFlags, installFlags } from '../platform/flags.ts';
import { clearDebugReport, debugReport, installDebug } from '../platform/debug.ts';
import { $, buttonClass, copyRich, download, esc } from './ui.ts';
import { copyFeedback } from './feedback.ts';
import { icon } from './icons.ts';

export function installDeveloperTools() {
    installFlags();
    installDebug();
    const link = document.createElement('button'),
        separator = document.createElement('span');
    link.type = 'button';
    link.className = 'su-footer-link';
    link.textContent = 'Developer tools';
    separator.textContent = ' · ';
    separator.setAttribute('aria-hidden', 'true');
    $('privacyOpen').parentElement!.append(separator, link);
    const dialog = document.createElement('dialog');
    dialog.className = 'su-dialog mars-modal developer-dialog';
    dialog.id = 'developerDialog';
    dialog.setAttribute('aria-labelledby', 'developerDialogTitle');
    dialog.innerHTML = `<header class="su-dialog-header"><h2 id="developerDialogTitle">Developer tools</h2><button type="button" class="su-dialog-close" aria-label="Close">${icon('cross')}</button></header><div class="su-dialog-body"></div><footer class="mars-modal-foot"></footer>`;
    document.body.append(dialog);
    dialog.querySelector<HTMLButtonElement>('.su-dialog-close')!.onclick = () => dialog.close();
    const render = () => {
        const active = dialog.contains(document.activeElement) ? (document.activeElement as HTMLElement) : null;
        const flags = featureFlags.list(),
            debug = featureFlags.any('debug'),
            experimental = featureFlags.any('experimental');
        link.hidden = separator.hidden = !flags.length;
        dialog.querySelector('.su-dialog-body')!.innerHTML =
            `<p>Active flag rules:</p>${flags.length ? `<ul>${flags.map((flag) => `<li><code>${esc(flag)}</code></li>`).join('')}</ul>` : '<p>No developer flags are enabled.</p>'}<div class="developer-flag-controls" role="group" aria-label="Flag controls"></div><p class="sync-fineprint">Flags apply to this browser profile and update other open tabs. They do not grant server permissions.</p>${debug ? '<p class="sync-fineprint">Debug reports stay local and exclude document text, names, URLs, credentials, and raw errors. Turning off debug also clears captured reports.</p>' : ''}`;
        const foot = dialog.querySelector('footer')!;
        const button = (label: string, run: (button: HTMLButtonElement) => void | Promise<void>, primary = false) => {
            const element = document.createElement('button');
            element.type = 'button';
            element.className = buttonClass({ primary, empty: !primary, text: !primary });
            element.textContent = label;
            element.onclick = () => {
                void run(element);
            };
            return element;
        };
        dialog
            .querySelector('.developer-flag-controls')!
            .replaceChildren(
                ...(debug ? [button('Turn off debug', () => featureFlags.disable('debug'))] : []),
                ...(experimental ? [button('Turn off experimental', () => featureFlags.disable('experimental'))] : []),
                ...(flags.length ? [button('Turn off all flags', () => featureFlags.clear())] : []),
            );
        foot.replaceChildren(
            ...(debug ? [button('Clear report', () => clearDebugReport())] : []),
            ...(debug
                ? [
                      button(
                          'Copy debug report',
                          async (button) => {
                              const report = debugReport();
                              try {
                                  await copyRich(report);
                                  copyFeedback(button);
                              } catch {
                                  download('MARS-debug.json', report, 'application/json');
                              }
                          },
                          true,
                      ),
                  ]
                : []),
        );
        if (active && !active.isConnected) dialog.querySelector<HTMLButtonElement>('.su-dialog-close')!.focus();
    };
    featureFlags.onChange(render);
    render();
    link.onclick = () => {
        render();
        dialog.showModal();
    };
    Object.assign(window, {
        marsDebug: {
            report: debugReport,
            clear: clearDebugReport,
            open: () => {
                render();
                if (!dialog.open) dialog.showModal();
            },
        },
    });
}
