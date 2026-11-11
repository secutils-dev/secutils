import { icon } from '../ui/icons.ts';
import { buttonClass } from '../ui/ui.ts';
import { hideTooltip } from '../ui/feedback.ts';

export interface FlowButton {
    label: string;
    value: string;
    primary?: boolean;
    danger?: boolean;
    empty?: boolean;
    left?: boolean;
    run?(button: HTMLButtonElement): void | Promise<void>;
}
export interface FlowScreen {
    title: string;
    html: string;
    back?: boolean;
    buttons?: FlowButton[];
    field?: { label: string; value: string; type?: 'text' | 'password'; readonly?: boolean };
}
export class SyncDialog {
    readonly element = document.createElement('dialog');
    readonly signal: AbortSignal;
    private controller = new AbortController();
    private heading = document.createElement('h2');
    private body = document.createElement('div');
    private foot = document.createElement('footer');
    private back = document.createElement('button');
    private notice = document.createElement('p');
    private input: HTMLInputElement | null = null;
    private answer: ((value: { value: string; text: string }) => void) | null = null;
    private resetBusy: (() => void) | null = null;
    constructor(id = 'syncDialog') {
        this.signal = this.controller.signal;
        this.element.className = 'su-dialog mars-modal sync-dialog';
        this.element.id = id;
        this.heading.id = `${id}Title`;
        this.heading.tabIndex = -1;
        this.element.setAttribute('aria-labelledby', this.heading.id);
        const header = document.createElement('header'),
            close = document.createElement('button');
        header.className = 'su-dialog-header';
        this.back.type = close.type = 'button';
        this.back.className = `${buttonClass({ empty: true, text: true, flush: 'left' })} sync-back`;
        this.back.innerHTML = `${icon('chevronRight')}<span>Back</span>`;
        this.back.setAttribute('aria-label', 'Back');
        this.back.onclick = () => this.resolve('back');
        close.className = 'su-dialog-close';
        close.innerHTML = icon('cross');
        close.setAttribute('aria-label', 'Close');
        close.onclick = () => this.close();
        this.body.className = 'su-dialog-body';
        this.foot.className = 'mars-modal-foot';
        this.notice.className = 'sync-notice';
        this.notice.setAttribute('role', 'status');
        header.append(this.heading, close);
        this.element.append(header, this.body, this.notice, this.foot);
        document.body.append(this.element);
        this.element.addEventListener('close', () => {
            this.controller.abort();
            this.resolve('close');
            this.element.remove();
        });
        this.element.addEventListener('cancel', (event) => {
            event.preventDefault();
            if (!this.back.hidden && !this.back.disabled) this.resolve('back');
            else this.close();
        });
    }
    get open() {
        return !this.signal.aborted;
    }
    screen(screen: FlowScreen) {
        if (!this.open) return Promise.resolve({ value: 'close', text: '' });
        this.idle();
        hideTooltip();
        this.resolve('replaced');
        this.heading.textContent = screen.title;
        this.back.hidden = !screen.back;
        this.body.innerHTML = screen.html;
        this.notice.classList.remove('sr-only');
        this.notice.textContent = '';
        this.input = null;
        if (screen.field) {
            const label = document.createElement('label'),
                input = document.createElement('input');
            label.className = 'field-label';
            label.textContent = screen.field.label;
            input.id = `${this.element.id}Input`;
            label.htmlFor = input.id;
            input.className = 'field';
            input.type = screen.field.type ?? 'text';
            input.value = screen.field.value;
            input.readOnly = !!screen.field.readonly;
            input.autocomplete = 'off';
            input.spellcheck = false;
            this.body.append(label, input);
            this.input = input;
            input.onkeydown = (event) => {
                if (event.key === 'Enter' && !event.isComposing && !input.readOnly) {
                    event.preventDefault();
                    this.resolve(screen.buttons?.find((button) => button.primary)?.value ?? 'back');
                }
            };
        }
        const button = (choice: FlowButton, index: number) => {
            const element = document.createElement('button');
            element.type = 'button';
            element.textContent = choice.label;
            const empty =
                !choice.primary &&
                !choice.danger &&
                (choice.empty ?? ['', 'back', 'close', 'cancel'].includes(choice.value));
            element.className = `${buttonClass({ ...choice, empty, text: empty, flush: empty ? (choice.left ? 'left' : index === (screen.buttons?.length ?? 0) - 1 ? 'right' : undefined) : undefined })}${choice.left ? ' sync-footer-left' : ''}`;
            element.onclick = () => {
                if (choice.run)
                    void Promise.resolve(choice.run(element)).catch(() => this.message('Could not finish. Try again.'));
                else this.resolve(choice.value);
            };
            return element;
        };
        this.foot.replaceChildren(this.back, ...(screen.buttons ?? []).map(button));
        this.foot.hidden = !screen.back && !screen.buttons?.length;
        for (const element of this.body.querySelectorAll<HTMLButtonElement>('[data-sync-action]'))
            element.onclick = () => this.resolve(element.dataset.syncAction!);
        if (!this.element.open) this.element.showModal();
        if (this.input && !this.input.readOnly) this.input.focus();
        else this.heading.focus({ preventScroll: true });
        return new Promise<{ value: string; text: string }>((resolve) => {
            this.answer = resolve;
        });
    }
    resolve(value: string) {
        const answer = this.answer;
        this.answer = null;
        answer?.({ value, text: this.input?.value.trim() ?? '' });
    }
    message(text: string) {
        if (this.open) {
            this.notice.classList.remove('sr-only');
            this.notice.textContent = text;
        }
    }
    busy(text: string) {
        if (this.resetBusy) return;
        const controls = [
            ...this.element.querySelectorAll<
                HTMLButtonElement | HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement
            >('button, input, select, textarea'),
        ].filter((control) => !control.classList.contains('su-dialog-close'));
        const states = controls.map((control) => [control, control.disabled] as const);
        const active = this.foot.querySelector<HTMLButtonElement>('.btn-primary, .btn-danger'),
            markup = active?.innerHTML;
        this.resetBusy = () => {
            for (const [control, disabled] of states) if (control.isConnected) control.disabled = disabled;
            if (active?.isConnected) {
                active.innerHTML = markup!;
                delete active.dataset.loading;
                active.removeAttribute('aria-busy');
            }
        };
        this.notice.classList.add('sr-only');
        this.notice.textContent = text;
        this.element.setAttribute('aria-busy', 'true');
        const action = this.foot.querySelector<HTMLButtonElement>('.btn-primary, .btn-danger');
        if (action) {
            action.dataset.loading = 'true';
            action.setAttribute('aria-busy', 'true');
            action.innerHTML = '<span class="su-spinner" aria-hidden="true"></span>';
            action.append(document.createTextNode(text));
        }
        for (const button of this.body.querySelectorAll<HTMLButtonElement>('button')) button.disabled = true;
        for (const button of this.foot.querySelectorAll<HTMLButtonElement>('button')) button.disabled = true;
        for (const input of this.body.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>(
            'input, select, textarea',
        ))
            input.disabled = true;
        this.back.disabled = true;
    }
    idle() {
        this.resetBusy?.();
        this.resetBusy = null;
        this.element.removeAttribute('aria-busy');
        this.back.disabled = false;
        this.notice.classList.remove('sr-only');
        this.notice.textContent = '';
    }
    close() {
        hideTooltip();
        this.controller.abort();
        this.resolve('close');
        if (this.element.open) this.element.close();
        this.element.remove();
    }
}
