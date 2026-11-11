export type ReadingWidth = number | 'full';
export type ExportFormat = 'html' | 'pdf';
export interface HtmlSettings { readingWidth: ReadingWidth; toc: boolean; find: boolean; pdf: boolean; source: boolean }
export interface PdfSettings { paper: 'A4' | 'Letter'; orientation: 'portrait' | 'landscape' }
export interface ExportSettings { v: 1; html: HtmlSettings; pdf: PdfSettings }
export const STORAGE_KEY = 'mars.exportSettings';

export const validWidth = (value: unknown): value is ReadingWidth => value === 'full'
    || (typeof value === 'number' && Number.isInteger(value) && value >= 320 && value <= 2400);
export const htmlDefaults = (width: ReadingWidth = 800): HtmlSettings => ({ readingWidth: validWidth(width) ? width : 800, toc: true, find: true, pdf: true, source: false });
export const pdfDefaults = (): PdfSettings => ({ paper: 'A4', orientation: 'portrait' });
export const defaults = (width: ReadingWidth = 800): ExportSettings => ({ v: 1, html: htmlDefaults(width), pdf: pdfDefaults() });

// Pure decoder: unknown versions and malformed values never become dialog state.
export function decodeSettings(raw: string | null, width: ReadingWidth = 800): ExportSettings {
    const out = defaults(width);
    try {
        const value = raw ? JSON.parse(raw) : null;
        if (value?.v !== 1) return out;
        if (validWidth(value.html?.readingWidth)) out.html.readingWidth = value.html.readingWidth;
        for (const key of ['toc', 'find', 'pdf', 'source'] as const) if (typeof value.html?.[key] === 'boolean') out.html[key] = value.html[key];
        if (value.pdf?.paper === 'Letter') out.pdf.paper = 'Letter';
        if (value.pdf?.orientation === 'landscape') out.pdf.orientation = 'landscape';
    } catch {}
    return out;
}

export function loadSettings(width: ReadingWidth): ExportSettings {
    try { return decodeSettings(localStorage.getItem(STORAGE_KEY), width); }
    catch { return defaults(width); }
}
export function commitSettings(settings: ExportSettings) {
    try { localStorage.setItem(STORAGE_KEY, JSON.stringify(settings)); } catch {}
}

export function pageDimensions(settings: PdfSettings) {
    const size = settings.paper === 'Letter' ? [215.9, 279.4] : [210, 297];
    const [width, height] = settings.orientation === 'landscape' ? size.toReversed() : size;
    return { width, height };
}
