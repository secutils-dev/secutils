import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import { runInNewContext } from 'node:vm';
import { buildSync } from 'esbuild';
import { analyticsConfig } from '../src/platform/analytics.ts';

test('analytics allowlists pageviews and redacts document URLs, referrers, and properties', () => {
    for (const hash of ['s=secret', 'live=secret', 'sync=secret', 'invite=secret', 'name=Private&md=content', 'b=bundle', 'doc=private-id']) {
        const config = analyticsConfig(new URL(`https://mars.example/private?url=https://private.example/token#${hash}`), 'https://referrer.example/private?token=secret#content');
        const input = { n: 'pageview', u: 'https://private.example/#secret', d: 'wrong', r: 'private', v: 37,
            p: { title: 'Private document' }, $: { amount: 'secret', currency: 'secret' }, h: 1, extra: 'secret' };
        assert.deepEqual(config.transformRequest!(input), { n: 'pageview', u: 'https://mars.example/', d: 'mars.example', r: 'https://referrer.example/', v: 37 });
        assert.equal(config.transformRequest!({ ...input, n: 'Private document name' }), null);
        assert.equal(config.hashBasedRouting, false);
        assert.equal(config.outboundLinks, false);
        assert.equal(config.fileDownloads, false);
        assert.equal(config.formSubmissions, false);
        assert.equal(config.bindToWindow, false);
    }
    for (const referrer of ['', 'not a URL', 'file:///private/file.md', 'data:text/plain,private']) {
        const config = analyticsConfig(new URL('https://mars.example/'), referrer);
        assert.equal(config.transformRequest!({ n: 'pageview', u: 'private', d: 'private' })!.r, null);
    }
});

test('the bundled Plausible tracker redacts initial, navigation, and engagement requests before fetch', () => {
    const script = buildSync({ entryPoints: [new URL('../src/platform/analytics.ts', import.meta.url).pathname],
        bundle: true, write: false, format: 'iife', globalName: 'analytics', target: 'es2023' }).outputFiles[0].text;
    const requests: { endpoint: string; payload: Record<string, unknown> }[] = [];
    const location = new URL('https://mars.example/?url=https://source.example/private#s=secret');
    const document = Object.assign(new EventTarget(), { body: { scrollHeight: 800 }, documentElement: { clientHeight: 800 },
        referrer: 'https://referrer.example/private?token=secret', visibilityState: 'visible', hasFocus: () => false });
    const window = Object.assign(new EventTarget(), { fetch: true, navigator: {}, localStorage: {}, location,
        history: { pushState() {} }, innerHeight: 800, scrollY: 0 });
    runInNewContext(`${script}\nanalytics.startAnalytics();`, { window, document, location, URL,
        ResizeObserver: class { observe() {} }, fetch: async (endpoint: string, init: RequestInit) => {
            requests.push({ endpoint, payload: JSON.parse(String(init.body)) }); return { status: 202 };
        } });
    assert.equal(requests.length, 1);
    location.href = 'https://mars.example/private-path?token=secret#live=secret';
    window.history.pushState();
    window.dispatchEvent(new Event('blur'));
    assert.ok(requests.some(request => request.payload.n === 'engagement'));
    assert.equal(requests.filter(request => request.payload.n === 'pageview').length, 2);
    for (const { endpoint, payload } of requests) {
        assert.equal(endpoint, 'https://tools.secutils.dev/api/event');
        assert.equal(payload.u, 'https://mars.example/');
        assert.equal(payload.d, 'mars.example');
        assert.ok(!JSON.stringify(payload).match(/secret|private|source\.example/));
        assert.ok(Object.keys(payload).every(key => ['n', 'u', 'd', 'r', 'v', 'e', 'sd'].includes(key)));
    }
});

test('the MARS shell never loads the legacy analytics script', () => {
    const html = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
    assert.equal(/<script[^>]+src=["'][^"']*\/js\/script[^"']*["']/i.test(html), false, 'Use the bundled, redacted tracker');
    assert.equal(/plausible\.init\(/.test(html), false, 'The legacy init shim cannot redact requests');
    assert.match(html, /<meta name="referrer" content="no-referrer">/);
});
