import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';
import { inlineSharedAssets } from '../dev/tools/shared/assets.ts';
import { assertInlineScriptSafety } from '../dev/tools/shared/html.ts';
import { ICONS, icon } from '../dev/tools/shared/icons.js';

test('inline script safety rejects HTML tokenizer traps without changing regex matching', () => {
    const unsafe = String.raw`globalThis.pattern = /<!-- su-comments/; globalThis.template = "<script>embedded<\/script>";`;
    assert.throws(() => assertInlineScriptSafety(`<script>${unsafe}</script>`, 'fixture'), /fixture: script #1.*comment opener/);
    const safe = unsafe.replace('<!--', '\\x3c!--');
    assertInlineScriptSafety(`<script>${safe}</script>`);
    assert.equal(runInNewContext(`${safe}\npattern.test('<!-- su-comments')`), true);
    assertInlineScriptSafety('<!-- ordinary document comment --><script>window.ready = true;</script>');
});

test('shared styles and glyph modules inline without external asset requests', () => {
  for (const name of ['echo', 'webhook']) {
    const source = readFileSync(new URL(`../dev/tools/${name}/index.html`, import.meta.url), 'utf8');
    const html = inlineSharedAssets(source);
    assert.match(html, /<style data-su-style="tokens">/);
    assert.match(html, /--eui-plain: #0b1628/);
    assert.match(html, /from 'data:text\/javascript;base64,/);
    assert.doesNotMatch(html, /href="\.\.\/shared\//);
    assert.doesNotMatch(html, /from '\.\.\/shared\//);
    assert.doesNotMatch(html, /src="\.\/assets\//);
    assert.match(html, /src="data:image\/svg\+xml;base64,/);
    const encoded = html.match(/from 'data:text\/javascript;base64,([^']+)'/)![1];
    assert.equal(Buffer.from(encoded, 'base64').toString(), readFileSync(new URL('../dev/tools/shared/icons.js', import.meta.url), 'utf8'));
    assert.equal(inlineSharedAssets(html), html);
  }
});

test('legacy pages and responder comments remain byte-identical', () => {
    const legacy = readFileSync(new URL('../dev/tools/jwt-debugger/index.html', import.meta.url), 'utf8');
    assert.equal(inlineSharedAssets(legacy), legacy);
    const comment = `<!-- @su:responder-script
<link rel="stylesheet" href="../shared/tokens.css" data-su-style="tokens">
<img src="./assets/echo-app-icon.svg" data-su-app-icon="echo">
<script type="module">import { icon } from '../shared/icons.js';</script>
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
    const template = readFileSync(new URL('../dev/tools/shared/og-template.html', import.meta.url), 'utf8');
    const bolt = template.match(/bolt:\s*'([^']+)'/)![1];
    assert.equal(svg.match(/<polygon[^>]*\/>/)![0], bolt.match(/<polygon[^>]*\/>/)![0]);
    assert.match(svg, /viewBox="0 0 80 80"/);
    assert.match(svg, /fill="#fed047"/);
    const source = '<img class="su-app-icon" src="./assets/echo-app-icon.svg"\n    data-su-app-icon="echo" width="32" height="32" alt="">';
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

test('the disconnect glyph matches EUI link_slash and button feedback primitives stay shared', () => {
    // Exact link_slash path from @elastic/eui 123.3.0. MARS CI does not install the Web UI.
    assert.equal(createHash('sha256').update(ICONS.unlink).digest('hex'), 'e3561c722c589e841fb2c46709a19a7fde35b59cf8ccd9145ced7c06d25b4ea4');
    const css = readFileSync(new URL('../dev/tools/shared/components.css', import.meta.url), 'utf8');
    assert.match(css, /\.btn-flush-left/);
    assert.match(css, /\.su-spinner/);
    assert.match(css, /prefers-reduced-motion: no-preference/);
    assert.match(css, /\.su-tooltip/);
});

test('the default-folder marker retains the EUI filled star at badge size', () => {
    assert.equal(createHash('sha256').update(ICONS.starFill).digest('hex'), '2fbceb67b18184b8e7212aca329f8092e528171f1c4ec907d27fbacecc9b58d0');
    assert.match(icon('starFill', 6), /width="6" height="6" viewBox="0 0 16 16"/);
});

test('the WEBHOOKS app icon uses SU colors and the Secutils Webhooks glyph', () => {
    const svg = readFileSync(new URL('../dev/tools/webhook/assets/webhook-app-icon.svg', import.meta.url), 'utf8');
    const template = readFileSync(new URL('../dev/tools/shared/og-template.html', import.meta.url), 'utf8');
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
