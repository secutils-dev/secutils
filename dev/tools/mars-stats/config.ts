// Deployment-only settings. The browser receives neither API credentials nor the access token.
import { createHash } from 'node:crypto';

export const MARS_STATS_SECRETS = { type: 'selected' as const, secrets: ['MARS_STATS_API_KEY'] };
export const MARS_STATS_HEADERS: [string, string][] = [
    ['Content-Type', 'text/html; charset=utf-8'],
    ['Cache-Control', 'no-store'],
    ['Referrer-Policy', 'no-referrer'],
    ['X-Content-Type-Options', 'nosniff'],
    ['X-Robots-Tag', 'noindex, nofollow'],
];

export function marsStatsCollectorSettings(toolsHost = 'tools.secutils.dev') {
    if (!/^[a-z0-9.-]+$/.test(toolsHost)) throw new Error('Invalid tools host');
    return {
        name: 'MARS Observatory — thirty-minute storage samples',
        enabled: true,
        notifications: false,
        secrets: MARS_STATS_SECRETS,
        config: { revisions: 1, job: {
            schedule: '0 */30 * * * *',
            retryStrategy: { type: 'exponential', initialInterval: 60000, multiplier: 2, maxInterval: 300000, maxAttempts: 3 },
        } },
        target: {
            url: `https://${toolsHost}/mars-stats?action=sample`,
            method: 'POST',
            configurator: `(() => {
                const key = context.params?.secrets?.MARS_STATS_API_KEY;
                if (!key) throw new Error('MARS_STATS_API_KEY secret is missing');
                return { requests: context.requests.map((request) => ({ ...request,
                    headers: { ...request.headers, Authorization: 'Bearer ' + key } })) };
            })()`,
        },
    };
}

export function configureMarsStatsScript(script: string, env = process.env): string {
    const responderId = env.SECUTILS_HTML_APP_RESPONDER_ID_MARS;
    const token = env.SECUTILS_MARS_STATS_VIEW_TOKEN;
    if (!responderId || !/^[a-f0-9-]{36}$/.test(responderId)) throw new Error('MARS responder ID is missing or invalid');
    if (!token || !/^[A-Za-z0-9_-]{43}$/.test(token) || Buffer.from(token, 'base64url').toString('base64url') !== token) {
        throw new Error('SECUTILS_MARS_STATS_VIEW_TOKEN must be a canonical base64url token of 32 random bytes');
    }
    const origin = new URL(env.SECUTILS_HTML_APP_API_DOMAIN || 'https://secutils.dev');
    if (origin.protocol !== 'https:' || origin.username || origin.password || origin.pathname !== '/' || origin.search || origin.hash) {
        throw new Error('The stats API domain must be an HTTPS origin');
    }
    return script
        .replaceAll('{{MARS_HOST}}', env.SECUTILS_MARS_PUBLIC_HOST || 'mars.secutils.dev')
        .replaceAll('{{MARS_RESPONDER_ID}}', responderId)
        .replaceAll('{{MARS_STATS_VIEW_TOKEN_HASH}}', createHash('sha256').update(Buffer.from(token, 'base64url')).digest('hex'))
        .replaceAll('{{SECUTILS_API_ORIGIN}}', origin.origin);
}
