// Export settings: their types, defaults, validation, and the stored copy in `localStorage`.
// Settings are a browser preference, separate from document state. The export dialog edits a
// draft and stores it only when the reader takes an output action (`commitSettings`).
// A stored value is untrusted: `decodeSettings` accepts each field only when it is valid.

/** A reading width in CSS pixels (an integer from 320 to 2400), or `full` for no limit. */
export type ReadingWidth = number | 'full';
export type ExportFormat = 'html' | 'pdf';
/** Options of the standalone HTML document. */
export interface HtmlSettings {
    readingWidth: ReadingWidth;
    /** Include a table of contents when the document has enough headings. */
    toc: boolean;
    /** Include Find in page. */
    find: boolean;
    /** Include the button that prepares a PDF from the standalone page. */
    pdf: boolean;
    /** Embed the Markdown source in the file. */
    source: boolean;
}
export interface PdfSettings {
    paper: 'A4' | 'Letter';
    orientation: 'portrait' | 'landscape';
}
/** The stored settings. `v` is the format version. Another version decodes to the defaults. */
export interface ExportSettings {
    v: 1;
    html: HtmlSettings;
    pdf: PdfSettings;
}
/** `localStorage` key of the JSON-encoded `ExportSettings`. */
export const STORAGE_KEY = 'mars.exportSettings';

/** Type guard for a reading width: `full`, or an integer from 320 to 2400 pixels. */
export const validWidth = (value: unknown): value is ReadingWidth =>
    value === 'full' || (typeof value === 'number' && Number.isInteger(value) && value >= 320 && value <= 2400);
/** HTML defaults with the given reading width. An invalid width becomes 800 pixels. */
export const htmlDefaults = (width: ReadingWidth = 800): HtmlSettings => ({
    readingWidth: validWidth(width) ? width : 800,
    toc: true,
    find: true,
    pdf: true,
    source: false,
});
export const pdfDefaults = (): PdfSettings => ({ paper: 'A4', orientation: 'portrait' });
export const defaults = (width: ReadingWidth = 800): ExportSettings => ({
    v: 1,
    html: htmlDefaults(width),
    pdf: pdfDefaults(),
});

/**
 * Pure decoder: unknown versions and malformed values never become dialog state.
 * Each field is read on its own. A missing or invalid field keeps its default. Never throws.
 *
 * @param raw The stored JSON, or null.
 * @param width The reading width for the defaults.
 */
export function decodeSettings(raw: string | null, width: ReadingWidth = 800): ExportSettings {
    const out = defaults(width);
    try {
        const value = raw ? JSON.parse(raw) : null;
        if (value?.v !== 1) return out;
        if (validWidth(value.html?.readingWidth)) out.html.readingWidth = value.html.readingWidth;
        for (const key of ['toc', 'find', 'pdf', 'source'] as const)
            if (typeof value.html?.[key] === 'boolean') out.html[key] = value.html[key];
        if (value.pdf?.paper === 'Letter') out.pdf.paper = 'Letter';
        if (value.pdf?.orientation === 'landscape') out.pdf.orientation = 'landscape';
    } catch {}
    return out;
}

/** Reads the stored settings. Returns the defaults when storage is unavailable. */
export function loadSettings(width: ReadingWidth): ExportSettings {
    try {
        return decodeSettings(localStorage.getItem(STORAGE_KEY), width);
    } catch {
        return defaults(width);
    }
}
/** Stores the settings as the preference for the next export. A storage failure is ignored. */
export function commitSettings(settings: ExportSettings) {
    try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    } catch {}
}

/** Page width and height in millimeters for the paper size and orientation. */
export function pageDimensions(settings: PdfSettings) {
    const size = settings.paper === 'Letter' ? [215.9, 279.4] : [210, 297];
    const [width, height] = settings.orientation === 'landscape' ? size.toReversed() : size;
    return { width, height };
}
