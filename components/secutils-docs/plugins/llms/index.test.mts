import assert from 'node:assert/strict';
import { before, describe, it } from 'node:test';

import { createRenderer, type RenderContext, type RenderedHtml } from './index.ts';

const CONTEXT: RenderContext = { pageUrl: 'https://secutils.dev/docs/guides/webhooks', siteTitle: 'Secutils.dev' };

let render: (html: string, context: RenderContext) => RenderedHtml;

before(async () => {
  render = await createRenderer();
});

/**
 * Wraps a doc body in the surrounding markup Docusaurus emits. Fixtures below are copied from the real built HTML,
 * since the transforms key off theme class names - a fixture that drifts from Docusaurus' output tests nothing.
 */
function page(body: string, title = 'Webhooks | Secutils.dev', description = 'Learn how to use webhooks.'): string {
  return [
    '<!DOCTYPE html><html><head>',
    `<title>${title}</title>`,
    `<meta data-rh="true" name="description" content="${description}"/>`,
    '</head><body><article>',
    '<nav class="theme-doc-breadcrumbs"><a href="/docs/">Docs</a></nav>',
    `<div class="theme-doc-markdown markdown">${body}</div>`,
    '<nav class="pagination-nav">Next page</nav>',
    '</article></body></html>',
  ].join('');
}

const escapeHtml = (value: string) => value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

/** Prism renders one `token-line` per line of code, and the copy button sits outside the `<pre>`. */
const codeBlock = (language: string, lines: string[]) =>
  [
    `<div class="language-${language} codeBlockContainer_Ckt0 theme-code-block">`,
    '<div class="codeBlockContent_QJqH">',
    `<pre tabindex="0" class="prism-code language-${language} codeBlock_bY9V"><code class="codeBlockLines_e6Vv">`,
    ...lines.map((line) => `<div class="token-line"><span class="token plain">${escapeHtml(line)}</span><br></div>`),
    '</code></pre></div>',
    '<div class="buttonGroup__atx"><button type="button" aria-label="Copy code to clipboard">Copy</button></div>',
    '</div>',
  ].join('');

describe('page metadata', () => {
  it('strips the site title suffix and reads the description meta', () => {
    const { title, description } = render(page('<p>Body.</p>'), CONTEXT);

    assert.equal(title, 'Webhooks');
    assert.equal(description, 'Learn how to use webhooks.');
  });

  it('converts only the doc body, not the surrounding breadcrumbs and pagination', () => {
    assert.equal(render(page('<p>Body.</p>'), CONTEXT).markdown, 'Body.');
  });

  it('fails loudly when the doc body container is missing', () => {
    assert.throws(() => render('<html><body><p>No container.</p></body></html>', CONTEXT), /theme-doc-markdown/);
  });
});

describe('headings', () => {
  it('drops the anchor link appended to every heading', () => {
    const html = page('<h2>What is a webhook?<a class="hash-link" href="#what" title="Direct link">&#8203;</a></h2>');

    assert.equal(render(html, CONTEXT).markdown, '## What is a webhook?');
  });
});

describe('code blocks', () => {
  it('rebuilds Prism token lines into a fenced block carrying the language', () => {
    const html = page(codeBlock('html', ['<html lang="en">', '<body>Hello World</body>', '</html>']));

    assert.equal(
      render(html, CONTEXT).markdown,
      ['```html', '<html lang="en">', '<body>Hello World</body>', '</html>', '```'].join('\n'),
    );
  });

  it('omits the copy button and emits a bare fence for the "text" pseudo-language', () => {
    const { markdown } = render(page(codeBlock('text', ['HTML Responder'])), CONTEXT);

    assert.equal(markdown, '```\nHTML Responder\n```');
    assert.ok(!markdown.includes('Copy'), 'the copy button label leaked into the code block');
  });
});

describe('labelled tables', () => {
  const sampleFields = (rows: string[]) =>
    `<div class="su-sample-fields"><table class="su-table"><tbody>${rows.join('')}</tbody></table></div>`;

  it('turns two-column label/value rows into labelled fenced blocks', () => {
    const html = page(
      sampleFields([
        `<tr><td><b>Name</b></td><td>${codeBlock('text', ['HTML Responder'])}</td></tr>`,
        `<tr><td><b>Headers</b></td><td>${codeBlock('http', ['Content-Type: text/html'])}</td></tr>`,
      ]),
    );

    assert.equal(
      render(html, CONTEXT).markdown,
      [
        '- **Name**:',
        '',
        '  ```',
        '  HTML Responder',
        '  ```',
        '',
        '- **Headers**:',
        '',
        '  ```http',
        '  Content-Type: text/html',
        '  ```',
      ].join('\n'),
    );
  });

  it('leaves a table alone when it is not a two-column grid', () => {
    const rows = ['<tr><th>A</th><th>B</th><th>C</th></tr>', '<tr><td>1</td><td>2</td><td>3</td></tr>'];

    assert.match(render(page(sampleFields(rows)), CONTEXT).markdown, /^\| A\s+\| B\s+\| C\s+\|$/m);
  });
});

describe('steps', () => {
  const step = (index: number, image: string, caption: string) =>
    [
      '<div class="su-steps__step">',
      `<div class="su-steps__indicator">${index}</div>`,
      '<div class="su-steps__content">',
      `<img src="${image}" alt="${caption}" loading="lazy" class="su-steps__img" role="button" tabindex="0">`,
      `<div class="su-steps__caption">${caption}</div>`,
      '</div></div>',
    ].join('');

  const steps = [
    '<div class="su-steps">',
    step(1, '../img/docs/guides/webhooks/html_step1_empty.png', 'Click Create responder.'),
    step(2, '../img/docs/guides/webhooks/html_step2_form.png', 'Fill in the form.'),
    '<dialog class="su-steps__lightbox"><img class="su-steps__lightbox-img"></dialog>',
    '</div>',
  ].join('');

  it('becomes an ordered list of image plus caption', () => {
    assert.equal(
      render(page(steps), CONTEXT).markdown,
      [
        '1. ![Click Create responder.](https://secutils.dev/docs/img/docs/guides/webhooks/html_step1_empty.png)',
        '',
        '   Click Create responder.',
        '',
        '2. ![Fill in the form.](https://secutils.dev/docs/img/docs/guides/webhooks/html_step2_form.png)',
        '',
        '   Fill in the form.',
      ].join('\n'),
    );
  });

  it('drops the step indicators and the lightbox dialog', () => {
    const { markdown } = render(page(steps), CONTEXT);

    // The lightbox `<img>` has neither `src` nor `alt`, so it would serialize as an empty `![]()` image.
    assert.equal(markdown.match(/!\[/g)?.length, 2);
    assert.ok(!markdown.includes('![]('), 'the lightbox emitted a stray image');
    assert.ok(!/^\s*1\s*$/m.test(markdown), 'a step indicator leaked as its own line');
  });
});

describe('admonitions', () => {
  const admonition = (kind: string, label: string, body: string) =>
    [
      `<div class="theme-admonition theme-admonition-${kind} admonition_xJq3 alert alert--success">`,
      '<div class="admonitionHeading_Gvgb"><span class="admonitionIcon_Rf37">',
      '<svg viewBox="0 0 12 16"><path d="M6.5 0C3.48 0 1 2.19 1 5"></path></svg>',
      `</span>${label}</div>`,
      `<div class="admonitionContent_BuS1">${body}</div>`,
      '</div>',
    ].join('');

  it('becomes a blockquote labelled with the admonition kind', () => {
    const html = page(admonition('tip', 'tip', '<p>Use a responder.</p>'));

    assert.equal(render(html, CONTEXT).markdown, '> **Tip**\n>\n> Use a responder.');
  });

  it('keeps a custom title and never leaks the icon', () => {
    const html = page(admonition('note', 'Absolute lifespan backstop', '<p>Body.</p>'));
    const { markdown } = render(html, CONTEXT);

    assert.match(markdown, /^> \*\*Absolute lifespan backstop\*\*$/m);
    assert.ok(!markdown.includes('M6.5 0C3.48'), 'the admonition icon path leaked as text');
  });
});

describe('urls', () => {
  it('resolves relative targets against the route without a trailing slash', () => {
    // From `/docs/guides/webhooks` this is `/docs/img/...`; a trailing slash would add a `guides/` level and 404.
    const html = page('<p><img src="../img/docs/guides/webhooks/x.png" alt="Shot"></p>');

    assert.equal(render(html, CONTEXT).markdown, '![Shot](https://secutils.dev/docs/img/docs/guides/webhooks/x.png)');
  });

  it('rewrites root-relative links but leaves absolute targets and fragments alone', () => {
    const html = page(
      '<p><a href="/ws/webhooks__responders">App</a> <a href="https://example.com/x">Ext</a>' +
        ' <a href="#top">Top</a></p>',
    );

    assert.equal(
      render(html, CONTEXT).markdown,
      '[App](https://secutils.dev/ws/webhooks__responders) [Ext](https://example.com/x) [Top](#top)',
    );
  });
});
