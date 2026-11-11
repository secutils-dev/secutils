import assert from 'node:assert/strict';
import test from 'node:test';
import {
    extractEmbeddedMarkdown,
    frontmatterTitle,
    readSource,
    sourceName,
    sourceUrl,
    stripSourceExtension,
    urlFileName,
} from '../src/io/source.ts';
import { InputController } from '../src/io/input.ts';
import { mergeDocuments, splitComments } from '../src/share/merge.ts';

test('source naming honors explicit names, frontmatter, headings, filenames, and fallback in order', () => {
    const text = '---\r\ntitle: "Frontmatter title"\r\n---\r\n# Heading\r\n\r\nBody.\r\n';
    assert.equal(sourceName(text, { name: ' Explicit.HTML ', fileName: 'File.md' }), 'Explicit');
    assert.equal(sourceName(text, { fileName: 'File.md' }), 'Frontmatter title');
    assert.equal(sourceName('# Heading ###\n\nBody.', { fileName: 'File.md' }), 'Heading');
    assert.equal(
        sourceName('```md\n# Example\n```\n\nActual title\n============', { fileName: 'File.html' }),
        'Actual title',
    );
    assert.equal(sourceName('Body only.', { fileName: 'File.HTM' }), 'File');
    assert.equal(sourceName('Body only.', { fallback: 'document' }), 'document');
    assert.equal(sourceName('Body only.'), 'Untitled');
    assert.equal(sourceName('\uFEFF# Unicode title\n'), 'Unicode title');
    for (const extension of ['md', 'MD', 'markdown', 'mdx', 'txt', 'html', 'htm'])
        assert.equal(stripSourceExtension(` File.${extension} `), 'File');
});

test('frontmatter titles support quoted scalars, CRLF, and empty/missing values', () => {
    assert.equal(frontmatterTitle('---\r\ntitle: "Quoted \\"title\\""\r\n---\r\nBody.'), 'Quoted "title"');
    assert.equal(frontmatterTitle("---\ntitle: 'Reader''s title'\n---\nBody."), "Reader's title");
    assert.equal(frontmatterTitle('---\ntitle: Plain title\n---'), 'Plain title');
    assert.equal(frontmatterTitle('---\ntitle: ""\n---\nBody.'), null);
    assert.equal(frontmatterTitle('---\ntitle: |\n  Multiple lines\n---\nBody.'), null);
    assert.equal(frontmatterTitle('# Heading'), null);
});

test('ordinary Markdown retains exact CRLF and review metadata, including source-tag examples', () => {
    const text =
        '---\r\ntitle: Doc\r\n---\r\n# Body\r\n\r\n<!-- su-comments\r\n[ { "id":"t", "quote":"Body", "comments":[{"body":"Review"}] } ]\r\n-->\r\n';
    assert.deepEqual(readSource(text, { fileName: 'File.md' }), { text, name: 'Doc', embedded: false });
    const example = '# Reader\n\n```html\n<script type="text/markdown">\nExample\n</script>\n```\n';
    assert.equal(readSource(example, { fileName: 'Reader.md' }).text, example);
    assert.equal(readSource(example, { fileName: 'Reader.md' }).embedded, false);
    const html = '<!DOCTYPE html><html><body><h1>Raw HTML</h1></body></html>';
    assert.deepEqual(readSource(html, { fileName: 'Reader.html' }), { text: html, name: 'Reader', embedded: false });
});

test('embedded source decoding reverses closing-script escaping and only the wrapping newlines', () => {
    const markup = '<html><script id="su-md-source" TYPE = text/markdown>source</script></html>';
    assert.equal(
        extractEmbeddedMarkdown(markup, () => '\n# Body\n\nLiteral <\\/script> and <\\/SCRIPT>.\n\n'),
        '# Body\n\nLiteral </script> and </SCRIPT>.\n',
    );
    assert.equal(
        extractEmbeddedMarkdown(markup, () => '\r\nBody\r\n\r\n'),
        'Body\r\n',
    );
    assert.equal(
        extractEmbeddedMarkdown(markup, () => ''),
        '',
    );
    assert.equal(
        extractEmbeddedMarkdown(markup, () => null),
        null,
    );
    assert.equal(
        extractEmbeddedMarkdown('<script type="text/plain">Source</script>', () => {
            throw new Error('Must not parse');
        }),
        null,
    );
});

test('URL validation is shared and filename decoding cannot discard a successful import', () => {
    assert.equal(sourceUrl('HTTPS://EXAMPLE.TEST/Reader.md').href, 'https://example.test/Reader.md');
    assert.equal(urlFileName(sourceUrl('https://example.test/Read%20me.HTML')), 'Read me.HTML');
    assert.equal(urlFileName(sourceUrl('https://example.test/bad%zz.md')), 'bad%zz.md');
    assert.equal(urlFileName(sourceUrl('https://example.test/')), 'document');
    for (const url of ['bad url', 'file:///Reader.md', 'javascript:alert(1)', 'ftp://example.test/Reader.md'])
        assert.throws(() => sourceUrl(url));
});

test('input generations abort superseded work and reject stale asynchronous completions', async () => {
    const inputs = new InputController();
    const first = inputs.begin();
    assert.equal(first.current(), true);
    let aborted = 0;
    first.signal.addEventListener('abort', () => aborted++);
    const second = inputs.begin();
    assert.equal(first.current(), false);
    assert.equal(first.signal.aborted, true);
    assert.equal(aborted, 1);
    assert.throws(() => first.check(), { name: 'AbortError' });
    await Promise.resolve();
    second.check();
    inputs.cancel();
    assert.equal(second.current(), false);
    assert.throws(() => second.check(), { name: 'AbortError' });
    inputs.cancel();
    assert.equal(aborted, 1);
});

test('unchanged receives preserve original review block bytes and merges retain CRLF', () => {
    const text =
        '# Body\r\n\r\n<!-- su-comments\r\n[ { "id":"t", "quote":"Body", "comments":[{"id":"c","body":"Review"}] } ]\r\n-->\r\n';
    const same = mergeDocuments(text, text);
    assert.equal(same.mine, text);
    assert.equal(same.theirs, text);
    const updated = text.replace('"body":"Review"', '"body":"Review"},{"id":"c2","body":"Reply"');
    const merged = mergeDocuments(text, updated);
    assert.equal(merged.theirs, updated);
    assert.equal(splitComments(merged.mine).threads[0].comments.length, 2);
    assert.doesNotMatch(merged.mine, /(?<!\r)\n/);
});
