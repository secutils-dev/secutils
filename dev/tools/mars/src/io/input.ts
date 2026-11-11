// One owner for asynchronous document input. Storage writes that already started can
// finish, but a superseded operation must never select or replace another document.
export interface InputOperation {
    readonly signal: AbortSignal;
    current(): boolean;
    check(): void;
}

export class InputController {
    private active: AbortController | null = null;

    begin(): InputOperation {
        this.cancel();
        const controller = new AbortController();
        this.active = controller;
        const current = () => this.active === controller && !controller.signal.aborted;
        return { signal: controller.signal, current, check: () => {
            if (!current()) throw new DOMException('Document input was superseded', 'AbortError');
        } };
    }

    cancel() {
        const previous = this.active;
        this.active = null;
        previous?.abort();
    }
}

const inputs = new InputController();
export const beginInput = () => inputs.begin();
export const cancelInput = () => inputs.cancel();
export const checkInput = (operation?: InputOperation) => operation?.check();
