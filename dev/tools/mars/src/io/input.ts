// One owner for asynchronous document input. Storage writes that already started can
// finish, but a superseded operation must never select or replace another document.
//
// Open, drops, imports, Load URL, links, and sidebar navigation each start an operation.
// A new operation cancels the previous one, so at most one is current (a "generation").
// Contract for a caller: pass the operation through each receive step. Check it after each
// asynchronous wait, and before a change to the selection or to a document.

/** One generation of document input. It is current until another operation begins or `cancelInput()` runs. */
export interface InputOperation {
    /** Aborts when the operation is superseded. Pass it to `fetch` and to dialogs. */
    readonly signal: AbortSignal;
    /** True while this operation is the active one. */
    current(): boolean;
    /** Throws a `DOMException` named `AbortError` when the operation is not current. */
    check(): void;
}

/** Holds the active operation. The app uses the one module-level instance below. */
export class InputController {
    private active: AbortController | null = null;

    /** Cancels the active operation and returns a new current one. */
    begin(): InputOperation {
        this.cancel();
        const controller = new AbortController();
        this.active = controller;
        const current = () => this.active === controller && !controller.signal.aborted;
        return {
            signal: controller.signal,
            current,
            check: () => {
                if (!current()) throw new DOMException('Document input was superseded', 'AbortError');
            },
        };
    }

    /** Aborts the active operation, if any. Its `current()` then returns false. */
    cancel() {
        const previous = this.active;
        this.active = null;
        previous?.abort();
    }
}

const inputs = new InputController();
/** Starts an input operation on the app-wide controller. This cancels the previous operation. */
export const beginInput = () => inputs.begin();
/** Cancels the current operation without a replacement, for example when the document closes. */
export const cancelInput = () => inputs.cancel();
/** `operation.check()` for an optional operation. A missing operation never throws. */
export const checkInput = (operation?: InputOperation) => operation?.check();
