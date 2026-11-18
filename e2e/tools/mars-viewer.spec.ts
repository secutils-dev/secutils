import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { deflateRawSync } from 'node:zlib';

import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

// Renderer, block-editing, and comment behavior of the shared viewer (src/doc/viewer.js).
// Nothing here writes to the MARS service, so the plain Playwright fixture is enough.
const MARS_BASE_URL = process.env.MARS_BASE_URL ?? 'https://mars.secutils.dev';
test.use({ baseURL: MARS_BASE_URL });

const source = (page: Page) =>
  page.evaluate(() => (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText());

// A full link, as the skill's `mars` shell function builds it.
async function open(page: Page, md: string, name = 'Viewer') {
  const utf8 = Buffer.from(md);
  const state = Buffer.concat([Buffer.alloc(4), deflateRawSync(utf8)]);
  state.writeUInt32LE(utf8.length, 0);
  await page.goto(`/#${new URLSearchParams({ name, md: state.toString('base64url') })}`);
  await expect(page.locator('#docTitle')).toHaveText(name);
}

async function downloadMarkdown(page: Page) {
  await page.getByRole('button', { name: 'Export document', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: /^Export / });
  const pending = page.waitForEvent('download');
  await dialog.getByRole('button', { name: 'Download Markdown', exact: true }).click();
  const text = await readFile(await (await pending).path(), 'utf8');
  await dialog.getByRole('button', { name: 'Close export', exact: true }).click();
  return text;
}

test.describe('viewer', () => {
  test('keeps the complete disclosure body collapsed and renders Markdown without an extra blank line', async ({
    page,
  }) => {
    const md = await readFile(resolve(__dirname, 'fixtures/mars-details.md'), 'utf8');
    await open(page, md, 'TODO');
    const preview = page.locator('#previewArticle');
    const details = preview.locator('details');
    await expect(details).toHaveCount(1);
    await expect(details.locator(':scope > summary')).toHaveText('Section Title');
    await expect(preview.getByText('The distinction is between', { exact: false })).toBeHidden();
    await details.locator('summary').click();
    await expect(details.locator('p > strong').first()).toContainText(
      'A reviewer found a reproducible formatting issue',
    );
    await expect(details.locator('ul > li')).toHaveCount(2);
    await expect(details.locator('strong > code')).toHaveText('terms');
    await expect(details.locator('blockquote')).toContainText('Yeah, good catch.');
    await expect(details.getByRole('link', { name: 'Terms query' })).toHaveAttribute(
      'href',
      'https://www.elastic.co/docs/reference/query-languages/query-dsl/query-dsl-terms-query',
    );
    expect(await source(page)).toBe(md);
    expect(await downloadMarkdown(page)).toBe(md);
  });

  test('GitHub alerts and ==highlights== are enhanced', async ({ page }) => {
    await open(page, '> [!NOTE]\n> Heads up.\n\nSome ==marked== text.');
    const preview = page.locator('#previewArticle');
    await expect(preview.locator('.markdown-alert-note')).toBeVisible();
    await expect(preview.locator('.markdown-alert-title')).toContainText('Note');
    await expect(preview.locator('mark.su-hl')).toContainText('marked');
  });

  test('disclosure summaries keep inline formatting, keyboard toggles, and nested body headings', async ({ page }) => {
    const md = [
      '<details>',
      '<summary>**Outer** `label`</summary>',
      '## Body heading',
      '',
      '<details>',
      '<summary>*Inner*</summary>',
      '- Nested **item**',
      '</details>',
      '',
      'Last paragraph.',
      '</details>',
      '',
      '## Outside heading',
    ].join('\n');
    await open(page, md);
    const article = page.locator('#previewArticle');
    const outer = article.locator('details').first();
    const summary = outer.locator(':scope > summary');
    await expect(summary.locator('strong')).toHaveText('Outer');
    await expect(summary.locator('code')).toHaveText('label');
    await expect(outer.getByRole('heading', { name: 'Body heading' })).toBeHidden();
    await summary.focus();
    await page.keyboard.press('Enter');
    await expect(outer.getByRole('heading', { name: 'Body heading' })).toBeVisible();
    const inner = outer.locator('details');
    await expect(inner.locator('li')).toBeHidden();
    await inner.locator('summary').click();
    await expect(inner.locator('li strong')).toHaveText('item');
    await expect(article.locator(':scope > .su-block > h2')).toContainText('Outside heading');
    await summary.focus();
    await page.keyboard.press('Space');
    await expect(outer.getByText('Last paragraph.')).toBeHidden();
    expect(await source(page)).toBe(md);
  });

  test('disclosure HTML cannot forge source ranges or run scripts', async ({ page }) => {
    await open(
      page,
      '<details open class="su-block" data-start="99" onclick="window.disclosureAttack = true">\n' +
        '<summary id="forged" data-end="999" onfocus="window.disclosureAttack = true">**Safe**</summary>\n' +
        '## Kept heading\n\n<script>window.disclosureAttack = true;</script>\n\n' +
        '<a href="javascript:window.disclosureAttack=true">Unsafe link</a>\n\n**Kept body**\n</details>',
    );
    const article = page.locator('#previewArticle');
    const details = article.locator('details');
    await expect(details.getByRole('heading', { name: 'Kept heading' })).toBeVisible();
    await expect(details.locator('p > strong')).toHaveText('Kept body');
    expect(
      await details.evaluate((el) => ({
        attributes: [...el.attributes].map((attribute) => attribute.name),
        summary: [...el.querySelector('summary')!.attributes].map((attribute) => attribute.name),
        unsafe: el.querySelectorAll('script, [onclick], [onfocus], a[href^="javascript:"]').length,
        ran: (window as unknown as { disclosureAttack?: boolean }).disclosureAttack ?? false,
      })),
    ).toEqual({ attributes: ['open'], summary: [], unsafe: 0, ran: false });
  });

  test('highlights inline code in tables', async ({ page }) => {
    await open(
      page,
      [
        '| Behaviour | Workflows | Dashboards |',
        '| --- | --- | --- |',
        '| Field name | `access_control` | `accessControl` |',
        '| What the restrictive mode restricts | visibility | ==writes only - everyone still reads== |',
        '| Default for new objects | ==`public`== | `default` |',
      ].join('\n'),
    );
    const preview = page.locator('#previewArticle');
    await expect(preview.locator('mark.su-hl')).toHaveText(['writes only - everyone still reads', 'public']);
    await expect(preview.getByRole('cell', { name: 'public', exact: true }).locator('mark.su-hl code')).toBeVisible();
    await expect(preview).not.toContainText('==');
  });

  test('highlights preserve nested inline formatting and literal code delimiters', async ({ page }) => {
    await open(
      page,
      'Before ==use `a == b` with **bold**, *emphasis*, and [a link](https://example.com)== after.\n\n' +
        '==``a ` b == c``== and ==another==.',
    );
    const marks = page.locator('#previewArticle mark.su-hl');
    await expect(marks).toHaveText(['use a == b with bold, emphasis, and a link', 'a ` b == c', 'another']);
    await expect(marks.first().locator('code')).toHaveText('a == b');
    await expect(marks.first().locator('strong')).toHaveText('bold');
    await expect(marks.first().locator('em')).toHaveText('emphasis');
    await expect(marks.first().getByRole('link', { name: 'a link' })).toHaveAttribute('href', 'https://example.com');
  });

  test('keeps code, escaped, empty, and unmatched highlight markers literal', async ({ page }) => {
    await open(
      page,
      [
        '`==inline code==`',
        '```text\n==fenced code==\n```',
        '    ==indented code==',
        '<code>==HTML code==</code>',
        '\\==escaped\\==',
        'Empty == == and ==== stay intact before ==highlighted==.',
        'Unmatched ==marker',
      ].join('\n\n'),
    );
    const preview = page.locator('#previewArticle');
    await expect(preview.locator('mark.su-hl')).toHaveText(['highlighted']);
    await expect(preview.locator('code')).toHaveText([
      '==inline code==',
      '==fenced code==\n',
      '==indented code==\n',
      '==HTML code==',
    ]);
    await expect(preview).toContainText('==escaped==');
    await expect(preview).toContainText('Empty == == and ==== stay intact before highlighted.');
    await expect(preview).toContainText('Unmatched ==marker');
  });

  test('unmatched inline HTML stays literal without swallowing the rest of the document', async ({ page }) => {
    await open(
      page,
      [
        '"no restyling <Button> from outside" with errors to steer agents.',
        '',
        'Unmatched <span class="example"> and </em> stay visible.',
        '',
        'Paired <em>HTML</em> and a break<br>still work.',
        '',
        '## Next section',
        '',
        '- A later item',
      ].join('\n'),
    );
    const preview = page.locator('#previewArticle');
    await expect(preview).toContainText('"no restyling <Button> from outside"');
    await expect(preview).toContainText('Unmatched <span class="example"> and </em> stay visible.');
    await expect(preview.locator('em')).toHaveText(['HTML']);
    await expect(preview.locator('br')).toHaveCount(1);
    await expect(preview.locator(':scope > .su-block > h2')).toContainText('Next section');
    await expect(preview.locator(':scope > .su-block > ul > li')).toHaveText(['A later item']);
  });

  test('block edits splice the source, act on sections, undo, and survive a reload', async ({ page }) => {
    const md = [
      '# Review',
      '',
      '## 1. First',
      '',
      'Keep this paragraph.',
      '',
      '- [ ] Task A',
      '- [ ] Task B',
      '',
      '## 2. Second',
      '',
      'Second body.',
      '',
      '## 3. Third',
      '',
      'Third body.',
      '',
    ].join('\n');
    await open(page, md);
    const article = page.locator('#previewArticle');
    const blockAction = async (block: ReturnType<typeof article.locator>, name: string) => {
      await block.hover();
      await page.locator('#blockGripBtn').click();
      await page.getByRole('menuitem', { name, exact: true }).click();
    };

    // Toggling a checkbox changes exactly one character.
    await article.locator('li', { hasText: 'Task A' }).locator('input[type="checkbox"]').click();
    let expected = md.replace('- [ ] Task A', '- [x] Task A');
    await expect.poll(() => source(page)).toBe(expected);
    await expect(page.locator('#docStats')).toContainText('4 of 5 left');

    // Done on a heading strikes it through and collapses its section.
    await blockAction(article.locator('h2', { hasText: '2. Second' }), 'Mark done');
    expected = expected.replace('## 2. Second', '## ~~2. Second~~');
    await expect.poll(() => source(page)).toBe(expected);
    await expect(article.getByText('Second body.')).toBeHidden();
    await expect(page.locator('#docStats')).toContainText('3 of 5 left');

    // Deleting a section leaves every other byte untouched, and undo restores it.
    await blockAction(article.locator('h2', { hasText: '3. Third' }), 'Delete section');
    await expect.poll(() => source(page)).toBe(expected.replace('\n## 3. Third\n\nThird body.\n', ''));
    await page.locator('#docStats').click();
    await page.keyboard.press('ControlOrMeta+z');
    await expect.poll(() => source(page)).toBe(expected);

    // Double-click edits a block as Markdown; Escape commits and renders it again.
    await article.getByText('Keep this paragraph.').dblclick();
    await expect(page.locator('.su-block-editor')).toBeVisible();
    await page.keyboard.press('ControlOrMeta+a');
    await page.keyboard.type('Edited **paragraph**.');
    await page.keyboard.press('Escape');
    expected = expected.replace('Keep this paragraph.', 'Edited **paragraph**.');
    await expect.poll(() => source(page)).toBe(expected);
    await expect(article.locator('strong')).toHaveText('paragraph');

    // "+" on a task item adds a sibling task with a prefilled marker.
    await article.locator('li', { hasText: 'Task B' }).hover();
    await page.locator('#blockAddBtn').click();
    await page.keyboard.type('Task C');
    await page.keyboard.press('Escape');
    expected = expected.replace('- [ ] Task B', '- [ ] Task B\n- [ ] Task C');
    await expect.poll(() => source(page)).toBe(expected);

    await expect(page.locator('#saveState')).toHaveText('Saved');
    await page.reload();
    await expect(article.locator('li', { hasText: 'Task C' })).toBeVisible();
    await expect.poll(() => source(page)).toBe(expected);
  });

  test('block edits keep CRLF line endings and frontmatter', async ({ page }) => {
    const md = '---\r\ntitle: Win\r\n---\r\n# Win\r\n\r\n- [ ] Task A\r\n- [ ] Task B\r\n';
    await open(page, md, 'Win');
    await page.locator('#previewArticle li', { hasText: 'Task A' }).locator('input[type="checkbox"]').click();
    await expect(page.locator('#saveState')).toHaveText('Saved');
    expect(await downloadMarkdown(page)).toBe(md.replace('- [ ] Task A', '- [x] Task A'));
  });

  test('comments render Markdown, keep the body byte-identical, and switch display modes', async ({ page }) => {
    const md = '# Review\n\nI disagree with warnings as the final behavior.\n\nSecond paragraph.\n';
    await open(page, md, 'Replies');
    await expect(page.locator('#previewArticle p').first()).toContainText('warnings as the final behavior');
    const highlighted = () =>
      page.evaluate(() =>
        ['su-comment', 'su-comment-active'].reduce((n, name) => n + (CSS.highlights.get(name)?.size ?? 0), 0),
      );

    await page.evaluate((needle) => {
      const p = [...document.querySelectorAll('#previewArticle p')].find((el) => el.textContent!.includes(needle))!;
      const text = p.firstChild as Text;
      const from = text.data.indexOf(needle);
      const range = document.createRange();
      range.setStart(text, from);
      range.setEnd(text, from + needle.length);
      getSelection()!.removeAllRanges();
      getSelection()!.addRange(range);
    }, 'warnings as the final behavior');
    await page.locator('#formatBar').getByRole('button', { name: 'Comment' }).click();
    const draft = page.locator('.cm-card.draft');
    await draft.getByLabel('Your name').fill('Oleg Zasypkin');
    await draft.getByRole('textbox', { name: 'Comment' }).fill('Too **strong**?');
    await page.keyboard.press('ControlOrMeta+Enter');
    const card = page.locator('.cm-card');
    await expect(card.locator('.cm-body strong')).toHaveText('strong');
    await expect(card.locator('.cm-avatar').first()).toHaveText('OZ');
    await expect.poll(highlighted).toBe(1);

    // Comments use the document renderer, diagrams included.
    const reply = 'Keep it.\n\n```mermaid\ngraph TD; A-->B\n```';
    await card.getByRole('textbox', { name: 'Reply' }).fill(reply);
    await card.getByRole('button', { name: 'Reply' }).click();
    await expect(card.locator('.cm-comment')).toHaveCount(2);
    await expect(card.locator('figure.su-mermaid svg')).toBeVisible({ timeout: 15000 });

    // The body stays byte-identical; threads follow it in one trailing block.
    await expect(page.locator('#saveState')).toHaveText('Saved');
    const saved = await downloadMarkdown(page);
    expect(saved.startsWith(md)).toBe(true);
    const threads = JSON.parse(saved.match(/\n<!-- su-comments\n([\s\S]*)\n-->\n$/)![1]);
    expect(threads).toHaveLength(1);
    expect(threads[0]).toMatchObject({
      quote: 'warnings as the final behavior',
      prefix: expect.stringMatching(/I disagree with $/),
    });
    expect(threads[0].comments.map((c: { body: string; author: string }) => [c.author, c.body])).toEqual([
      ['Oleg Zasypkin', 'Too **strong**?'],
      ['Oleg Zasypkin', reply],
    ]);

    // A reload restores the threads; clicking the highlight opens the thread.
    await page.reload();
    await expect(card.locator('.cm-comment')).toHaveCount(2);
    await expect.poll(highlighted).toBe(1);
    const { x, y } = await page.evaluate(() => {
      const range = [...CSS.highlights.get('su-comment')!][0] as Range;
      const r = range.getBoundingClientRect();
      return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
    });
    await page.mouse.click(x, y);
    await expect(card).toHaveClass(/active/);

    // The quote toggles a thread; collapsed mode folds every inactive thread to one line.
    const title = card.locator('.cm-quote');
    await title.click();
    await expect(title).toHaveAttribute('aria-expanded', 'false');
    await title.click();
    await expect(title).toHaveAttribute('aria-expanded', 'true');
    const modeButton = page.locator('#commentsBtn');
    await modeButton.click();
    await expect(modeButton).toHaveAttribute('data-mode', 'collapsed');
    await page.keyboard.press('Escape');
    await expect(title).toHaveAttribute('aria-expanded', 'false');
    await expect(card.locator('.cm-count')).toHaveText('2');
    await modeButton.click();
    await expect(page.locator('#commentsRail')).toBeHidden();
    await expect.poll(highlighted).toBe(0);
    await modeButton.click();
    await expect(modeButton).toHaveAttribute('data-mode', 'expanded');

    await card.getByRole('button', { name: 'Resolve thread' }).click();
    await expect(card).toHaveClass(/resolved/);
    await expect.poll(highlighted).toBe(0);
  });
});
