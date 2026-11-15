// Sidebar import pickers capture their destination before the native picker opens.
// Each picker owns its pending target. Cancellation clears it without changing the selected folder.
import * as app from '../app.ts';
import { $ } from '../ui/ui.ts';
import { importFiles } from './io.ts';

const PICKERS = { files: 'importInput', folder: 'importFolderInput' } as const;
const destinations = new WeakMap<HTMLInputElement, string | null>();

/** Opens a native import picker synchronously and keeps `parentId` until selection or cancellation. */
export function pickImport(kind: keyof typeof PICKERS, parentId: string | null) {
    const input = $<HTMLInputElement>(PICKERS[kind]);
    destinations.set(input, parentId);
    input.click();
}

/** Installs the two sidebar pickers once at startup. Repeated selections of the same files fire again. */
export function installImportPickers() {
    for (const id of Object.values(PICKERS)) {
        const input = $<HTMLInputElement>(id);
        input.addEventListener('cancel', () => destinations.delete(input));
        input.addEventListener('change', () => {
            const files = [...(input.files ?? [])];
            const parentId = destinations.has(input) ? destinations.get(input)! : app.state.folderId;
            destinations.delete(input);
            input.value = '';
            if (files.length) void importFiles(files, parentId);
        });
    }
}
