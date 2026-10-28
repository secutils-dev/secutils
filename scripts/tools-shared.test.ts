import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { inlineSharedAssets } from '../dev/tools/shared-assets.ts';
import { ICONS, icon } from '../dev/tools/shared/icons.js';

test('shared styles and glyph modules inline without external asset requests', () => {
  for (const name of ['echo', 'webhook']) {
    const source = readFileSync(new URL(`../dev/tools/${name}.html`, import.meta.url), 'utf8');
    const html = inlineSharedAssets(source);
    assert.match(html, /<style data-su-style="tokens">/);
    assert.match(html, /--eui-plain: #0b1628/);
    assert.match(html, /from 'data:text\/javascript;base64,/);
    assert.doesNotMatch(html, /href="\.\/shared\//);
    assert.doesNotMatch(html, /from '\.\/shared\//);
    assert.doesNotMatch(html, /src="\.\/(echo|webhook)\/assets\//);
    assert.match(html, /src="data:image\/svg\+xml;base64,/);
    const encoded = html.match(/from 'data:text\/javascript;base64,([^']+)'/)![1];
    assert.equal(Buffer.from(encoded, 'base64').toString(), readFileSync(new URL('../dev/tools/shared/icons.js', import.meta.url), 'utf8'));
    assert.equal(inlineSharedAssets(html), html);
  }
});

test('legacy pages and responder comments remain byte-identical', () => {
    const legacy = readFileSync(new URL('../dev/tools/jwt-debugger.html', import.meta.url), 'utf8');
    assert.equal(inlineSharedAssets(legacy), legacy);
    const comment = `<!-- @su:responder-script
<link rel="stylesheet" href="./shared/tokens.css" data-su-style="tokens">
<img src="./echo/assets/echo-app-icon.svg" data-su-app-icon="echo">
<script type="module">import { icon } from './shared/icons.js';</script>
-->`;
    assert.equal(inlineSharedAssets(comment), comment);
});

test('shared asset markers reject unknown names and mismatched paths', () => {
    assert.throws(() => inlineSharedAssets('<link data-su-style="unknown">'), /Unknown/);
    assert.throws(() => inlineSharedAssets('<link rel="stylesheet" href="../tokens.css" data-su-style="tokens">'), /must reference/);
    assert.throws(() => inlineSharedAssets('<img src="../icon.svg" data-su-app-icon="echo">'), /must reference/);
});

test('the standalone ECHO app icon preserves the OG bolt artwork', () => {
    const svg = readFileSync(new URL('../dev/tools/echo/assets/echo-app-icon.svg', import.meta.url), 'utf8');
    const template = readFileSync(new URL('../dev/tools/og-template.html', import.meta.url), 'utf8');
    const bolt = template.match(/bolt:\s*'([^']+)'/)![1];
    assert.equal(svg.match(/<polygon[^>]*\/>/)![0], bolt.match(/<polygon[^>]*\/>/)![0]);
    assert.match(svg, /viewBox="0 0 80 80"/);
    assert.match(svg, /fill="#fed047"/);
    const source = '<img class="su-app-icon" src="./echo/assets/echo-app-icon.svg"\n    data-su-app-icon="echo" width="32" height="32" alt="">';
    const html = inlineSharedAssets(source);
    assert.doesNotMatch(html, /data-su-app-icon=/);
    assert.equal(inlineSharedAssets(html), html);
    const embedded = html.match(/src="data:image\/svg\+xml;base64,([^"]+)"/)![1];
    assert.equal(Buffer.from(embedded, 'base64').toString(), svg);
});

test('EUI glyphs retain their grid, color, and decorative accessibility contract', () => {
    for (const name of Object.keys(ICONS) as (keyof typeof ICONS)[]) {
        const svg = icon(name);
        assert.match(svg, /fill="currentColor" aria-hidden="true"/);
        assert.doesNotMatch(svg, /undefined/);
        assert.match(svg, new RegExp(`viewBox="0 0 ${name.startsWith('logo') ? '32 32' : '16 16'}"`));
    }
    assert.throws(() => icon('missing' as keyof typeof ICONS), /Unknown EUI icon/);
});

test('the WEBHOOKS app icon uses SU colors and the Secutils Webhooks glyph', () => {
    const svg = readFileSync(new URL('../dev/tools/webhook/assets/webhook-app-icon.svg', import.meta.url), 'utf8');
    const template = readFileSync(new URL('../dev/tools/og-template.html', import.meta.url), 'utf8');
    const webhook = template.match(/webhook:\s*'([^']+)'/)![1];
    const paths = (source: string) => [...source.matchAll(/<path\s+[^>]*\/>/g)].map(([path]) => path.replace(/\s+/g, ' '));
    assert.deepEqual(paths(svg), paths(webhook));
    assert.equal(paths(svg).length, 1);
    assert.match(webhook, /viewBox="0 0 16 16"/);
    assert.match(svg, /fill="#fed047"/);
    assert.match(svg, /fill="#642340"/);
    assert.match(svg, /viewBox="0 0 80 80"/);
    const app = readFileSync(new URL('../components/secutils-webui/src/pages/workspace/utils/index.ts', import.meta.url), 'utf8');
    assert.match(app, /case UTIL_HANDLES.webhooksResponders:\s*return 'vectorTriangle'/);
});
