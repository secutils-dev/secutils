// Keep Plausible, but redact before transport. The legacy /js/script.js build
// ignores transformRequest and sends location.href, including fragment secrets.
//
// Owns the tracker configuration. The privacy contract: an event carries the site root URL,
// the host name, the referrer origin, the event name, and the tracker version. Nothing else.
// A fragment holds documents and link keys, so the page address never enters an event.
import { init, type PlausibleConfig } from '@plausible-analytics/tracker';

/**
 * Builds the tracker configuration for a page address and a referrer. Pure, so tests cover
 * the redaction without a browser.
 * Only `pageview` events pass. Automatic tracking of hash routes, outbound links, file
 * downloads, and form submissions is off, and the tracker gets no `window.plausible` binding.
 */
export function analyticsConfig(address: URL, referrer: string): PlausibleConfig {
    // Keep only the origin of an http(s) referrer. Its path and query can hold private data.
    let referral: string | null = null;
    try {
        const url = new URL(referrer);
        if (['https:', 'http:'].includes(url.protocol)) referral = `${url.origin}/`;
    } catch {}
    const domain = address.hostname,
        url = `${address.origin}/`;
    return {
        domain,
        // The self-hosted Plausible instance of Secutils.dev.
        endpoint: 'https://tools.secutils.dev/api/event',
        autoCapturePageviews: true,
        hashBasedRouting: false,
        outboundLinks: false,
        fileDownloads: false,
        formSubmissions: false,
        customProperties: {},
        bindToWindow: false,
        logging: false,
        transformRequest: (payload) => {
            // Null drops the event. Only page views are allowed.
            if (payload.n !== 'pageview') return null;
            // Rebuild an allowlisted payload. Never forward names, props, queries,
            // fragments, referrer paths, or future tracker fields by spreading it.
            return {
                n: 'pageview',
                u: url,
                d: domain,
                r: referral,
                ...(Number.isSafeInteger(payload.v) ? { v: payload.v } : {}),
            };
        },
    };
}

/** Starts the tracker. A failure is ignored: analytics must never stop the app. */
export function startAnalytics() {
    try {
        init(analyticsConfig(new URL(location.href), document.referrer));
    } catch {}
}
