// Keep Plausible, but redact before transport. The legacy /js/script.js build
// ignores transformRequest and sends location.href, including fragment secrets.
import { init, type PlausibleConfig } from '@plausible-analytics/tracker';

export function analyticsConfig(address: URL, referrer: string): PlausibleConfig {
    let referral: string | null = null;
    try {
        const url = new URL(referrer);
        if (['https:', 'http:'].includes(url.protocol)) referral = `${url.origin}/`;
    } catch {}
    const domain = address.hostname,
        url = `${address.origin}/`;
    return {
        domain,
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

export function startAnalytics() {
    try {
        init(analyticsConfig(new URL(location.href), document.referrer));
    } catch {}
}
