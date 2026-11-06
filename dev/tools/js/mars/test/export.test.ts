import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { build } from 'esbuild';
import { decodeSettings, defaults, pageDimensions, validWidth } from '../src/export/settings.ts';
import { exportBody, renderKey, snapshot } from '../src/export/snapshot.ts';

test('export settings restore typed values, reject malformed storage, and seed reading width', () => {
    assert.deepEqual(decodeSettings(null, 'full'), defaults('full'));
    for (const raw of ['broken', '{}', '{"v":2}', 'null', '[]']) assert.deepEqual(decodeSettings(raw, 1234), defaults(1234));
    assert.deepEqual(decodeSettings(JSON.stringify({ v: 1, html: { readingWidth: 320, toc: false, find: false, pdf: false, source: true }, pdf: { paper: 'Letter', orientation: 'landscape' } })), {
        v: 1, html: { readingWidth: 320, toc: false, find: false, pdf: false, source: true }, pdf: { paper: 'Letter', orientation: 'landscape' },
    });
    assert.deepEqual(decodeSettings('{"v":1,"html":{"readingWidth":"2400","toc":"false"},"pdf":{"paper":"A0"}}'), defaults());
    for (const width of [320, 800, 1100, 2400, 'full']) assert.equal(validWidth(width), true);
    for (const width of [0, 319, 2401, 1234.5, NaN, null, 'custom', '800']) assert.equal(validWidth(width), false);
});

test('export snapshots are immutable, omit frontmatter/review metadata, and retain saved model views', () => {
    const body = '## Body\r\n\r\n```stl view={"zoom":2}\r\nsolid model\r\nendsolid model\r\n```\r\n';
    const text = `---\r\ntitle: Secret metadata\r\n---\r\n${body}\r\n<!-- su-comments\r\n[{"quote":"Body","comments":[{"body":"Private review"}]}]\r\n-->\r\n`;
    const captured = snapshot('doc', 'Name', text, 1100);
    assert.equal(Object.isFrozen(captured), true);
    assert.equal(captured.text, text);
    assert.equal(captured.body, body.replaceAll('\r\n', '\n'));
    assert.equal(renderKey(captured), captured.body);
    assert.equal(exportBody('Text\n<!-- su-comments\nbroken json\n-->'), 'Text');
    assert.equal(exportBody('Ordinary <!-- HTML comment -->.'), 'Ordinary <!-- HTML comment -->.');
});

test('PDF dimensions preserve A4/Letter paper dimensions in both orientations', () => {
    assert.deepEqual(pageDimensions({ paper: 'A4', orientation: 'portrait' }), { width: 210, height: 297 });
    assert.deepEqual(pageDimensions({ paper: 'A4', orientation: 'landscape' }), { width: 297, height: 210 });
    assert.deepEqual(pageDimensions({ paper: 'Letter', orientation: 'portrait' }), { width: 215.9, height: 279.4 });
    assert.deepEqual(pageDimensions({ paper: 'Letter', orientation: 'landscape' }), { width: 279.4, height: 215.9 });
});

// Build pure output assembly with representative runtime strings. Browser cases below use
// the real separately bundled runtimes and the deploy-equivalent page.
const result = await build({
    stdin: { contents: "export * from './src/export/html.ts'; export * from './src/export/pdf.ts';", resolveDir: process.cwd() },
    bundle: true, write: false, format: 'esm', platform: 'node', loader: { '.css': 'text' },
    plugins: [{ name: 'test-runtime', setup(builder) {
        builder.onResolve({ filter: /^mars-runtime:/ }, ({ path }) => ({ path, namespace: 'test' }));
        builder.onLoad({ filter: /.*/, namespace: 'test' }, () => ({ contents: `export default ${JSON.stringify('console.log("reader</script>")')}`, loader: 'js' }));
    } }],
});
const { htmlDocument, embedSource, hasToc, printCss } = await import(`data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`);

test('HTML assembly escapes titles/source, retains reader features, and excludes app state', () => {
    const captured = snapshot('private-id', 'Title </title><script>bad()</script>', '## Body\n\nLiteral </script> text.', 800);
    const content = '<h1>Body</h1><h2>One</h2><h2>Two</h2><h3>Three</h3><pre><code>Code</code></pre>';
    const settings = defaults(); settings.html.source = true;
    const output = htmlDocument(captured, content, settings, 'operation');
    assert.match(output, /Title &lt;\/title&gt;&lt;script&gt;bad\(\)&lt;\/script&gt;/);
    assert.match(output, /id="toc"/);
    assert.match(output, /id="find-btn"/);
    assert.match(output, /id="pdf-btn"/);
    assert.match(output, /id="reading-settings"/);
    assert.match(output, /id="theme-toggle"/);
    assert.match(output, /Literal <\\\/script> text/);
    assert.match(output, /reader<\\\/script>/);
    assert.doesNotMatch(output, /private-id|IndexedDB|plausible|su-comments|su-block/);
    for (const key of ['toc', 'find', 'pdf', 'source'] as const) settings.html[key] = false;
    const minimal = htmlDocument(captured, content, settings, 'operation');
    assert.doesNotMatch(minimal, /id="(?:toc|find-btn|su-find|pdf-btn|su-md-source)"/);
    assert.equal(hasToc('<h2>One</h2>'), false);
    assert.match(embedSource('</SCRIPT><script>x</script>'), /<\\\/SCRIPT>/);
});

test('print CSS retains dimension, footer, light, and vector layout contracts', () => {
    for (const paper of ['A4', 'Letter'] as const) for (const orientation of ['portrait', 'landscape'] as const) {
        const dimensions = pageDimensions({ paper, orientation });
        assert.match(printCss({ paper, orientation }), new RegExp(`size: ${dimensions.width}mm ${dimensions.height}mm`));
    }
    assert.match(printCss({ paper: 'A4', orientation: 'portrait' }), /counter\(page\).*counter\(pages\)/);
    assert.match(printCss({ paper: 'A4', orientation: 'portrait' }), /string\(doctitle\)/);
    assert.doesNotMatch(printCss({ paper: 'A4', orientation: 'portrait' }), /:is\(h1/);
});

test('standalone runtimes have no app imports, function stringification, or eager PDF script', async () => {
    const reader = await readFile('src/export/reader-runtime.ts', 'utf8');
    const print = await readFile('src/export/print-runtime.ts', 'utf8');
    const html = await readFile('src/export/html.ts', 'utf8');
    for (const source of [reader, print, html]) assert.doesNotMatch(source, /app\.ts|store\/db|\.toString\(\)|plausible|transformers/);
    assert.match(print, /pagedjs@0\.4\.3/);
    assert.doesNotMatch(html, /<script src=/);
});
