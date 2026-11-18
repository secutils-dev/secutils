import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import { Marked } from 'marked';
import { markdownDetails } from '../src/doc/markdown-details.ts';

const md = new Marked({ extensions: [markdownDetails] });
const render = (source: string) => md.parse(source, { async: false });

test('the regression fixture parses as one complete disclosure without changing its source', () => {
    const source = readFileSync(new URL('../../../../e2e/tools/fixtures/mars-details.md', import.meta.url), 'utf8');
    const tokens = md.lexer(source);
    const details = tokens.find((token) => token.type === 'details')!;
    assert.equal(details.raw.trimEnd(), source.slice(source.indexOf('<details>')).trimEnd());
    const html = render(source);
    assert.match(html, /<details>\s*<summary>Section Title<\/summary><p><strong>A reviewer found/);
    assert.match(html, /<strong><code>terms<\/code> query<\/strong>/);
    assert.match(html, /<blockquote>[\s\S]*<\/blockquote>\n<\/details>/);
});

test('summary formatting, body headings, nested disclosures, and following prose stay distinct', () => {
    const source =
        '<details open>\n<summary>**Outer** <em>title</em></summary>\n## Inside\n\n<details>\n<summary>Inner</summary>\n*Nested*.\n</details>\n\nLast paragraph.\n</details>\n\n## Outside';
    const html = render(source);
    assert.match(html, /<summary><strong>Outer<\/strong> <em>title<\/em><\/summary><h2>Inside<\/h2>/);
    assert.match(html, /<details>\s*<summary>Inner<\/summary><p><em>Nested<\/em>\.<\/p>\n<\/details>/);
    assert.match(html, /<p>Last paragraph\.<\/p>\n<\/details><h2>Outside<\/h2>/);
    assert.deepEqual(
        md
            .lexer(source)
            .filter((token) => token.type !== 'space')
            .map((token) => token.type),
        ['details', 'heading'],
    );
});

test('literal disclosure tags in code, HTML raw text, comments, and attributes do not end the body', () => {
    for (const literal of [
        '```html\n</details>\n```',
        '~~~~html\n</details>\n~~~\n~~~~',
        '> ~~~html\n> </details>\n> ~~~',
        '- ~~~html\n  </details>\n  ~~~',
        '    </details>',
        '`</details>`',
        '``literal ` </details>``',
        '<pre>\n</details>\n</pre>',
        '<code></details></code>',
        '<!-- </details> -->',
        '<a title="</details>">Attribute text</a>',
        '<details title="a > b">\n<summary>Nested</summary>\nBody\n</details>',
    ]) {
        const source = `<details>\n<summary>Title</summary>\n\n${literal}\n\nFinal **paragraph**.\n</details>\n\nOutside.`;
        assert.equal(md.lexer(source)[0].raw, source.slice(0, source.lastIndexOf('</details>') + 10), literal);
        assert.match(
            render(source),
            /<p>Final <strong>paragraph<\/strong>\.<\/p>\n<\/details><p>Outside\.<\/p>/,
            literal,
        );
    }
});

test('unmatched disclosures and fenced examples keep the standard Markdown behavior', () => {
    assert.equal(md.lexer('<details>\n<summary>Title</summary>\nUnclosed.')[0].type, 'html');
    assert.equal(md.lexer('```html\n<details>\n<summary>Title</summary>\n</details>\n```')[0].type, 'code');
    assert.equal(md.lexer('<details>\n<summary>Title</summary>\n```html\n</details>')[0].type, 'html');
});

test('disclosure HTML passes through the host renderer instead of bypassing its policy', () => {
    const calls: string[] = [];
    const safe = new Marked({
        extensions: [markdownDetails],
        renderer: {
            html({ text }) {
                calls.push(text);
                return '';
            },
        },
    });
    const html = safe.parse('<details onclick="bad()">\n<summary>Title</summary>\n**Body**\n</details>', {
        async: false,
    });
    assert.deepEqual(calls, ['<details onclick="bad()">\n<summary>', '</summary>', '</details>']);
    assert.equal(html, 'Title<p><strong>Body</strong></p>\n');
});
