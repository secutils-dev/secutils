import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { discoverBundles, discoverTools, TOOLS_DIR } from '../dev/tools/shared/sources.ts';
import { TOOLS } from '../e2e/tools/registry.ts';

function fixture(t: { after(fn: () => void): void }) {
    const root = mkdtempSync(join(tmpdir(), 'secutils-tools-'));
    t.after(() => rmSync(root, { recursive: true, force: true }));
    const file = (name: string, value = '') => {
        const path = join(root, name);
        mkdirSync(join(path, '..'), { recursive: true });
        writeFileSync(path, value);
    };
    return { root, file };
}

test('discovery supports single HTML apps and app directories without scanning shared files or build artifacts', (t) => {
    const { root, file } = fixture(t);
    for (const name of ['plain.html', 'app/index.html', 'app/SKILL.md',
        'app/dist/example.html', 'app/node_modules/example/index.html', 'shared/index.html', 'shared/og-template.html']) file(name);
    assert.deepEqual(discoverTools(root), [
        { slug: 'app', html: 'app/index.html', skill: 'app/SKILL.md' },
        { slug: 'plain', html: 'plain.html' },
    ]);
    file('plain/index.html');
    assert.throws(() => discoverTools(root), /Duplicate tool source: plain/);
});

test('a companion skill requires an app directory instead of split top-level sources', (t) => {
    const { root, file } = fixture(t);
    file('plain.html');
    file('plain.skill.md');
    assert.throws(() => discoverTools(root), /app directory with index.html and SKILL.md/);
});

test('bundle names come from app-local or shared package metadata and reject ambiguous or missing names', (t) => {
    const { root, file } = fixture(t);
    file('app/index.html');
    file('app/package.json', JSON.stringify({ suBundle: 'private-bundle' }));
    file('shared/libs/common/package.json', JSON.stringify({ suBundle: 'common' }));
    assert.deepEqual(discoverBundles(root), [
        { name: 'common', directory: join(root, 'shared/libs/common') },
        { name: 'private-bundle', directory: join(root, 'app') },
    ]);
    file('shared/libs/common/package.json', JSON.stringify({ suBundle: 'private-bundle' }));
    assert.throws(() => discoverBundles(root), /Duplicate bundle name/);
    file('shared/libs/common/package.json', '{}');
    assert.throws(() => discoverBundles(root), /Missing or invalid suBundle/);
});

test('the real source inventory matches the E2E registry, skills, and public source links', () => {
    const sources = discoverTools();
    assert.deepEqual(sources.map((source) => source.html).sort(), TOOLS.map((tool) => tool.source).sort());
    for (const source of sources) {
        const tool = TOOLS.find((tool) => tool.source === source.html)!;
        const html = readFileSync(join(TOOLS_DIR, source.html), 'utf8');
        assert.ok(html.includes(`<meta name="su-tool-path" content="${tool.path}">`), source.html);
        if (source.slug === 'index') continue;
        if (tool.promote) assert.ok(source.skill, source.slug);
        if (source.skill) assert.ok(existsSync(join(TOOLS_DIR, source.skill)), source.slug);
        const schema = JSON.parse(html.match(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/)![1]);
        assert.equal(schema.sameAs, `https://github.com/secutils-dev/secutils/blob/main/dev/tools/${source.html}`);
    }
});

test('single-file sources stay flat, companion skills stay app-local, and bundle placeholders resolve uniquely', () => {
    const sources = discoverTools();
    assert.equal(sources.find((source) => source.slug === 'index')?.html, 'index.html');
    for (const source of sources.filter((source) => source.skill)) {
        assert.equal(source.html, `${source.slug}/index.html`);
        assert.equal(source.skill, `${source.slug}/SKILL.md`);
    }
    const bundles = discoverBundles();
    assert.deepEqual(bundles.map((bundle) => bundle.name), ['liteparse', 'mars', 'micro-ml', 'three-mf']);
    const used = new Set<string>();
    for (const { html } of sources) {
        for (const [, name] of readFileSync(join(TOOLS_DIR, html), 'utf8').matchAll(/data-su-bundle="([a-z0-9_-]+)"/g)) {
            assert.equal(bundles.filter((bundle) => bundle.name === name).length, 1, `${html}: ${name}`);
            used.add(name);
        }
    }
    assert.deepEqual([...used].sort(), bundles.map((bundle) => bundle.name));
});
