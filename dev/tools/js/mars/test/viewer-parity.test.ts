import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';

test('Mermaid navigation and styles match Markdown Preview', () => {
    const viewer = readFileSync(new URL('../src/doc/viewer.js', import.meta.url), 'utf8');
    const md = readFileSync(new URL('../../../md.html', import.meta.url), 'utf8');
    const css = readFileSync(new URL('../src/doc/doc.css', import.meta.url), 'utf8');
    const helper = (s: string) => s.slice(s.indexOf('function initMermaids('), s.indexOf('// Maps use SVG;'));
    const matching = (s: string) => s.slice(s.indexOf('function mermaidViews('), s.indexOf('// Keep SVG sizing'));
    const styles = (s: string) => s.slice(s.indexOf('/* Mermaid:'), s.indexOf('.su-diagram {'));
    assert.ok(helper(viewer).length > 100);
    assert.equal(helper(viewer), helper(md));
    assert.equal(matching(viewer), matching(md));
    assert.equal(styles(css), styles(md));
});

test('Mermaid views follow unchanged sources, including edited duplicates and reordered diagrams', () => {
    const viewer = readFileSync(new URL('../src/doc/viewer.js', import.meta.url), 'utf8');
    const source = viewer.slice(viewer.indexOf('function mermaidViews('), viewer.indexOf('// Keep SVG sizing'));
    const match = runInNewContext(`(${source})`);
    const root = (figures: object[]) => ({ querySelectorAll: () => figures });
    const old = ['A', 'B', 'A'].map((src, index) => ({ __suMermaidSource: src, __suReadMermaidView: () => ({ index }) }));
    for (const [sources, expected] of [
        [['A', 'B', 'A'], [0, 1, 2]],
        [['Changed', 'B', 'A'], [undefined, 1, 2]],
        [['A', 'B', 'Changed'], [0, 1, undefined]],
        [['B', 'A', 'A'], [1, 0, 2]],
        [['New', 'A', 'B', 'A'], [undefined, 0, 1, 2]],
        [[], []],
    ] as const) {
        const next = sources.map((src) => ({ __suMermaidSource: src }));
        const result = match(root(old), root(next));
        assert.deepEqual(next.map((fig) => result.get(fig)?.index), expected);
    }
});

test('preview links preserve anchors and existing link metadata in both viewers', () => {
    const viewer = readFileSync(new URL('../src/doc/viewer.js', import.meta.url), 'utf8');
    const md = readFileSync(new URL('../../../md.html', import.meta.url), 'utf8');
    const helper = (s: string) => s.slice(s.indexOf('function enhancePreviewLinks('), s.indexOf('// `comment` escapes raw HTML'));
    assert.ok(helper(viewer).length > 100);
    assert.equal(helper(viewer), helper(md));
    const attributes: Record<string, string>[] = [
        { href: 'https://example.test/', rel: 'nofollow opener', title: 'Original title', target: '_self' },
        { href: '/relative' },
        { href: '#section' },
    ];
    const links = attributes.map((attrs) => ({
        getAttribute: (name: string) => attrs[name] ?? null,
        setAttribute: (name: string, value: string) => { attrs[name] = value; },
    }));
    const enhance = runInNewContext(`(${helper(viewer)})`);
    enhance({ querySelectorAll: () => links });
    enhance({ querySelectorAll: () => links }); // Idempotent on rerender.
    assert.deepEqual(attributes, [
        { href: 'https://example.test/', rel: 'nofollow noopener noreferrer', title: 'Original title', target: '_blank' },
        { href: '/relative', target: '_blank', rel: 'noopener noreferrer' },
        { href: '#section' },
    ]);
});

test('both viewers parse only the final review block and preserve earlier Markdown examples', () => {
    const viewer = readFileSync(new URL('../src/doc/viewer.js', import.meta.url), 'utf8');
    const md = readFileSync(new URL('../../../md.html', import.meta.url), 'utf8');
    const section = (text: string) => {
        const start = text.indexOf('const COMMENTS_RE'), fn = text.indexOf('function matchCommentBlock(', start);
        return text.slice(start, text.indexOf('\n}', fn) + 2);
    };
    assert.equal(section(viewer), section(md));
    const match = runInNewContext(`${section(viewer)}\nmatchCommentBlock`);
    const body = '# Example\r\n\r\n```md\r\n<!-- su-comments\r\n[]\r\n-->\r\n```\r\n';
    const text = `${body}\r\n<!-- su-comments\r\n[{"id":"real-review"}]\r\n-->\r\n`;
    const result = match(text);
    assert.equal(text.slice(0, result.index), body);
    assert.equal(JSON.parse(result[1])[0].id, 'real-review');
});
