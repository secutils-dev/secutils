// The print document for PDF output, and the sandboxed preview frame with its message bridge.
// The export dialog and the standalone reader both use this module, so it imports no app,
// storage, or UI module.
// - A preview frame is a `srcdoc` iframe without `allow-same-origin`. Its origin is opaque: its
//   scripts cannot read the storage, cookies, or DOM of the page that embeds it.
// - An opaque origin has no name to use as a `postMessage` target, and its `event.origin`
//   (`null`) identifies no frame. The bridge accepts a message only from the window of its own
//   frame, with the fixed kind and the operation identifier of that frame. The one message to
//   a frame, `print`, carries no document content.
import printRuntime from 'mars-runtime:print';
import { head, jsonTag, scriptTag, watermark } from './document.ts';
import { pageDimensions, type PdfSettings } from './settings.ts';

/** Sandbox of the PDF frame: scripts for pagination, and modals for the print dialog of the browser. */
export const PRINT_SANDBOX = 'allow-scripts allow-modals';
/** The `kind` field of each bridge message. The runtimes in the frames use the same literal. */
export const MESSAGE_KIND = 'mars-export';

/**
 * The print stylesheet for a paper size: page boxes with a page counter and the document title,
 * break rules, and hidden interactive controls. Paged.js parses this CSS, so it stays within
 * what that parser supports.
 */
export function printCss(settings: PdfSettings) {
    const { width, height } = pageDimensions(settings);
    return `@page {
        size: ${width}mm ${height}mm; margin: 22mm 18mm 26mm;
        @bottom-center { content: counter(page) " / " counter(pages); font: 9pt "Inter", sans-serif; color: #6b7280; }
        @bottom-right { content: string(doctitle); font: 8pt "Inter", sans-serif; color: #98a2b3; max-width: 90mm; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    }
    @page :first { margin-top: 32mm; @bottom-right { content: ""; } }
    html, body { color-scheme: light; background: #fff; color: #1f2937; }
    body { font-size: 11pt; line-height: 1.6; }
    main, body.has-toc main { margin: 0; padding: 0; } main article { max-width: none; }
    article { string-set: doctitle attr(data-title); }
    article h1 { margin-top: 0; }
    article h1, article h2, article h3, article h4, article h5, article h6 { break-after: avoid-page; }
    article p, article li { orphans: 3; widows: 3; }
    article pre, article table, article blockquote, article img, article figure { break-inside: avoid; }
    article pre { white-space: pre-wrap; word-break: break-word; font-size: 9.5pt; }
    article table { font-size: 9.5pt; }
    article a { color: #642340; text-decoration: none; }
    article .su-anchor, .su-diagram-controls, .copy-btn, .su-diagram canvas { display: none !important; }
    .su-diagram .su-stl-snapshot { display: block; width: 100%; }
    .su-diagram .su-diagram-view, .su-diagram .su-stl-snapshot, .su-mermaid svg { max-height: ${height - 90}mm; object-fit: contain; }
    .su-diagram figcaption { font-size: 9pt; }
    .mdp-table-wrap { overflow: visible !important; }
    .su-watermark { border-top: 1px solid #e5e7eb; padding: 16pt 0 0; margin-top: 24pt; opacity: .55; break-before: avoid-page; }
    .pagedjs_margin-content { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
    `;
}

/**
 * Builds the print document for the PDF frame.
 * Uses the already-rendered content. Model PNGs and complete Mermaid SVGs are immutable.
 *
 * @param content Export-safe HTML of the body. Scripts, canvases, and controls are removed.
 * @param title The document name. It is escaped in the head and set through the DOM as an attribute.
 * @param operation The identifier that the print runtime puts on its messages.
 */
export function printDocument(content: string, title: string, settings: PdfSettings, operation: string) {
    // Content is export-safe renderer output, but discard interactive model payloads/controls.
    const root = new DOMParser().parseFromString(content, 'text/html');
    for (const el of root.querySelectorAll('script, canvas, .su-diagram-controls, .su-anchor, .copy-btn')) el.remove();
    for (const el of root.querySelectorAll('[tabindex]')) el.removeAttribute('tabindex');
    const article = root.createElement('article');
    article.dataset.title = title;
    article.append(...root.body.childNodes);
    return `<!DOCTYPE html><html lang="en" data-theme="light"><head>${head(title, content, printCss(settings))}</head><body><main>${article.outerHTML}</main>${watermark}${jsonTag('mars-print-config', { operation, ...pageDimensions(settings) })}${scriptTag(printRuntime)}</body></html>`;
}

/** A preview frame and its controls. */
export interface PreparedFrame {
    frame: HTMLIFrameElement;
    /**
     * Resolves when the frame reports `ready`. Rejects with the error text of the frame, after
     * 60 seconds without an answer, or with an `AbortError` when `dispose` runs first.
     */
    ready: Promise<void>;
    /** Asks the frame to open the print dialog. Does nothing before `ready` or after `dispose`. */
    print(): void;
    /** Removes the frame and the message listener. Safe to call more than once. */
    dispose(): void;
}

/**
 * Appends a sandboxed frame with `html` to `container` and connects the message bridge.
 * Opaque srcdoc: communicate through source-window + operation checks, never child globals.
 *
 * Messages from the frame: `ready` and `error` settle `ready` once. `print-requested` counts only
 * after `ready`, and `afterprint` only after a `print-requested`. Every other message is ignored.
 *
 * @param html The complete document for the frame, from `htmlDocument` or `printDocument`.
 * @param operation The identifier inside `html`. Use a fresh random value for each frame.
 * @param format `pdf` uses `PRINT_SANDBOX`. `html` also allows popups and downloads.
 * @param onPrint Runs when the frame is about to open the print dialog.
 * @param onAfterPrint Runs when that print dialog closed.
 */
export function prepareFrame(
    container: HTMLElement,
    html: string,
    operation: string,
    format: 'html' | 'pdf',
    onPrint?: () => void,
    onAfterPrint?: () => void,
): PreparedFrame {
    const frame = document.createElement('iframe');
    frame.title = format === 'html' ? 'Standalone HTML preview' : 'Paginated PDF preview';
    frame.sandbox.value = format === 'pdf' ? PRINT_SANDBOX : 'allow-scripts allow-modals allow-popups allow-downloads';
    frame.setAttribute('allow', 'fullscreen; clipboard-write');
    frame.referrerPolicy = 'no-referrer';
    let settled = false,
        succeeded = false,
        printing = false,
        dispose = () => {};
    const ready = new Promise<void>((resolve, reject) => {
        const fail = (error: Error) => {
            if (!settled) {
                settled = true;
                clearTimeout(timer);
                reject(error);
            }
        };
        const timer = setTimeout(
            () => fail(new Error('Preview preparation timed out. Retry when the network is available.')),
            60000,
        );
        const receive = (event: MessageEvent) => {
            // Any window can post to this page. Accept only this frame's window, and only a
            // message with this frame's operation identifier.
            if (
                event.source !== frame.contentWindow ||
                event.data?.kind !== MESSAGE_KIND ||
                event.data.operation !== operation
            )
                return;
            if (event.data.type === 'ready' && !settled) {
                settled = succeeded = true;
                clearTimeout(timer);
                resolve();
            } else if (event.data.type === 'error')
                fail(new Error(String(event.data.message || 'Preview preparation failed.')));
            else if (event.data.type === 'print-requested' && succeeded) {
                printing = true;
                onPrint?.();
            } else if (event.data.type === 'afterprint' && printing) {
                printing = false;
                onAfterPrint?.();
            }
        };
        window.addEventListener('message', receive);
        dispose = () => {
            fail(new DOMException('Preview cancelled', 'AbortError'));
            clearTimeout(timer);
            window.removeEventListener('message', receive);
            frame.remove();
        };
        container.append(frame);
        frame.srcdoc = html;
    });
    return {
        frame,
        ready,
        dispose: () => dispose(),
        print: () => {
            if (!succeeded || !frame.isConnected) return;
            // The opaque origin of the frame has no name, so the target origin is `*`.
            frame.contentWindow?.postMessage({ kind: MESSAGE_KIND, operation, type: 'print' }, '*');
        },
    };
}
