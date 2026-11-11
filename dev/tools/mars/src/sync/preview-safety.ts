// Connected previews treat all received raw HTML as untrusted. DOMPurify supplies the
// same policy in every supported engine; the native Sanitizer remains unavailable in Safari.
import DOMPurify from 'dompurify';

const FORBIDDEN = ['script', 'style', 'iframe', 'object', 'embed', 'base', 'meta', 'link', 'form', 'input', 'button', 'textarea', 'select', 'option', 'svg', 'math'];
export function safeRawHtml(html: string): string {
    return DOMPurify.sanitize(html, { USE_PROFILES: { html: true }, FORBID_TAGS: FORBIDDEN,
        FORBID_ATTR: ['style', 'id', 'name', 'class', 'srcset'], ALLOW_DATA_ATTR: false, ALLOW_ARIA_ATTR: false,
        SANITIZE_NAMED_PROPS: true });
}
export function safeRenderedHtml(html: string): string {
    // Source range attributes are allowed only AFTER raw HTML lost all data attributes.
    // The renderer, not the received document, supplies these attributes.
    return DOMPurify.sanitize(html, { USE_PROFILES: { html: true }, FORBID_TAGS: FORBIDDEN.filter((tag) => tag !== 'input'),
        FORBID_ATTR: ['style', 'name', 'srcset'], ALLOW_DATA_ATTR: true, SANITIZE_NAMED_PROPS: true });
}
