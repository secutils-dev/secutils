// Immutable KaTeX assets shared by the viewer and exported documents. Versions and SRI stay paired.

/** The KaTeX release used by every rendered math view. */
export const KATEX_VERSION = '0.19.1';
/** The stylesheet for the pinned KaTeX release. */
export const KATEX_CSS_URL = `https://cdn.jsdelivr.net/npm/katex@${KATEX_VERSION}/dist/katex.min.css`;
/** The SHA-384 digest of the pinned stylesheet, computed from its release bytes. */
export const KATEX_CSS_INTEGRITY = 'sha384-WwGtaC6ywCHxA5cstpj/3+xIZvTez8dq2IcBQaXRkenHZ7FVVq1ZROtURwfbtTlG';
/** The ES module for the pinned KaTeX release. */
export const KATEX_JS_URL = `https://cdn.jsdelivr.net/npm/katex@${KATEX_VERSION}/dist/katex.mjs`;
