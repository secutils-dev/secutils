interface MergeRange {
    from: number;
    to: number;
    active?: boolean;
}
interface MergeEditor {
    getText(): string;
    replace(from: number, to: number, insert: string): void;
    highlight(ranges: MergeRange[]): void;
    reveal(position: number): void;
    focus(): void;
    destroy(): void;
}

export function loadEditor(mount: HTMLElement): Promise<void>;
export function createMergeEditor(
    parent: HTMLElement,
    doc: string,
    options: {
        label: string;
        readOnly: boolean;
        onChange?(text: string): void;
        ranges?: MergeRange[];
    },
): Promise<MergeEditor>;
