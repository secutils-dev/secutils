// One settings form for folder Sync providers. Saving settings never runs Sync now.
import { hideTooltip, showTooltip } from '../ui/feedback.ts';
import { icon, type IconName } from '../ui/icons.ts';
import { syncBackButton, syncManagement } from './dialog.ts';
import { readSchedule, saveSchedule, type Cadence, type ScheduleOptions, type SyncSchedule } from './schedule.ts';

const SEND_HELP = 'MARS saves your edits here as you work. It sends them after you stop editing for the selected time.';

function label(value: Cadence) {
    if (value === 'manual') return 'Manual';
    if (value < 60_000) return `Every ${value / 1000} seconds`;
    return value === 60_000 ? 'Every minute' : `Every ${value / 60_000} minutes`;
}

/** Labeled scheduling controls for a native form. Use a unique literal prefix in each dialog. */
export function scheduleFields(prefix: string, github = false) {
    return `<label for="${prefix}Receive">Receive updates</label><select class="select" id="${prefix}Receive" name="receive"${github ? '' : ` aria-describedby="${prefix}Scope" autofocus`}></select>
        <div class="sync-interval-label"><label for="${prefix}Send">Send updates</label><button class="icon-btn sync-interval-help" id="${prefix}Help" type="button" aria-label="About send timing" data-tooltip-help>${icon('question')}</button></div>
        <select class="select" id="${prefix}Send" name="send" aria-describedby="${prefix}SendHelp${github ? '' : ` ${prefix}Scope`}"></select>
        <p id="${prefix}SendHelp" class="sync-fineprint"></p>
        ${github ? '' : `<p id="${prefix}Scope" class="sync-fineprint">These settings apply to this folder in this browser.</p>`}`;
}

/** Updates available choices in place and reads the draft without submitting. GitHub Never has no send interval. */
export function bindScheduleFields(dialog: HTMLElement, prefix: string, github = false) {
    const receive = dialog.querySelector<HTMLSelectElement>(`#${prefix}Receive`)!;
    const send = dialog.querySelector<HTMLSelectElement>(`#${prefix}Send`)!;
    const help = dialog.querySelector<HTMLButtonElement>(`#${prefix}Help`)!;
    const description = dialog.querySelector<HTMLElement>(`#${prefix}SendHelp`)!;
    help.onclick = () => showTooltip(help, description.textContent!);
    const cadence = (value: string): Cadence => (value === 'manual' ? 'manual' : Number(value));
    return {
        read(): SyncSchedule {
            return { receive: cadence(receive.value), send: send.value === 'never' ? undefined : cadence(send.value) };
        },
        render(options: ScheduleOptions, value?: Partial<SyncSchedule>) {
            receive.replaceChildren(
                ...options.receive.map((interval) => new Option(label(interval), String(interval))),
            );
            receive.value = String(options.receive.includes(value?.receive!) ? value!.receive : options.receive[0]);
            send.replaceChildren(
                ...(github ? [new Option('Never', 'never')] : []),
                ...(options.send ?? []).map((interval) => new Option(label(interval), String(interval))),
            );
            send.disabled = !options.send;
            send.value = options.send?.includes(value?.send!)
                ? String(value!.send)
                : github
                  ? 'never'
                  : String(options.send?.[0]);
            description.textContent = send.disabled ? '' : SEND_HELP;
            if (github) {
                const permission = document.createElement('strong');
                permission.textContent = 'Contents: Read and write';
                description.append(
                    `${send.disabled ? '' : ' '}Choose a credential with `,
                    permission,
                    ' to send updates.',
                );
            }
            description.classList.toggle('sr-only', !(github && send.disabled));
            help.dataset.tooltip = description.textContent;
        },
    };
}

/** Opens local settings. A management action carries the unsaved choices for the return trip. */
export function openSyncSchedule(
    schedule?: { key: string; options: ScheduleOptions },
    actions: readonly { label: string; value: string; icon: IconName }[] = [],
    draft?: SyncSchedule,
): Promise<{ action: string; draft?: SyncSchedule }> {
    const dialog = document.createElement('dialog');
    dialog.className = 'su-dialog mars-modal sync-dialog';
    dialog.setAttribute('aria-labelledby', 'syncScheduleTitle');
    dialog.innerHTML = `<header class="su-dialog-header"><h2 id="syncScheduleTitle">Sync settings</h2><button class="su-dialog-close" type="button" aria-label="Close">${icon('cross')}</button></header>
        <form method="dialog" aria-labelledby="syncScheduleTitle">
            <div class="su-dialog-body github-fields">
                ${schedule ? scheduleFields('syncTiming') : ''}${syncManagement(actions)}
                <p class="sync-error" role="alert" hidden></p>
            </div>
            <footer class="mars-modal-foot">${schedule ? '<button type="submit" class="btn btn-primary">Save</button>' : ''}</footer>
        </form>`;
    const fields = schedule ? bindScheduleFields(dialog, 'syncTiming') : undefined;
    if (schedule) fields?.render(schedule.options, draft ?? readSchedule(schedule.key, schedule.options));
    let action = '';
    for (const button of dialog.querySelectorAll<HTMLButtonElement>('[data-sync-action]'))
        button.onclick = () => {
            action = button.dataset.syncAction!;
            dialog.close();
        };
    dialog.querySelector('footer')!.prepend(syncBackButton(() => dialog.close()));
    dialog.querySelector('.su-dialog-close')!.addEventListener('click', () => dialog.close());
    dialog.querySelector('form')!.addEventListener('submit', (event) => {
        event.preventDefault();
        if (!schedule || !fields) return;
        try {
            saveSchedule(schedule.key, schedule.options, fields.read());
            dialog.close();
        } catch {
            const error = dialog.querySelector<HTMLElement>('.sync-error')!;
            error.textContent = 'Could not save Sync settings in this browser.';
            error.hidden = false;
        }
    });
    return new Promise((resolve) => {
        dialog.addEventListener(
            'close',
            () => {
                hideTooltip();
                resolve({ action, draft: action ? fields?.read() : undefined });
                dialog.remove();
            },
            { once: true },
        );
        document.body.append(dialog);
        hideTooltip();
        dialog.showModal();
    });
}
