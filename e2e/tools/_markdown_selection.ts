import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';
import { PNG } from 'pngjs';

const markdown = [
  'Plain words here.',
  '```text\nfenced code words\n```',
  '```js\nconst highlighted = true;\n```',
  '    indented code words',
  '| Column | Value |\n| --- | --- |\n| table words | cell words |',
  'Last paragraph.',
].join('\n\n');

// CodeMirror draws selections behind the lines. Computed styles can look correct
// while an opaque active-line background hides the selection, so check painted pixels.
export function sourceSelectionTests(load: (page: Page, markdown: string) => Promise<void>) {
  test.describe('source selection', () => {
    for (const theme of ['dark', 'light']) {
      test(`selected source text has a visible background (${theme})`, async ({ page }) => {
        const text = 'First line of plain source text.';
        await load(page, `${text}\nSecond line of plain source text.`);
        await page.evaluate((value) => document.documentElement.setAttribute('data-theme', value), theme);
        const content = page.locator('#editorMount .cm-content');
        await expect(content).toBeVisible(); // Require CodeMirror, not its textarea fallback.
        await content.click();
        await page.keyboard.press('ControlOrMeta+Home');
        const line = content.locator('.cm-line').first();
        await expect(line).toHaveClass(/cm-activeLine/);
        await page.evaluate(() => document.fonts.ready);
        const before = PNG.sync.read(await line.screenshot({ animations: 'disabled', caret: 'hide' }));

        await page.keyboard.press('Shift+End');
        await expect.poll(() => page.evaluate(() => getSelection()?.toString())).toBe(text);
        const selection = page.locator('#editorMount .cm-selectionBackground');
        await expect(selection).toBeVisible();
        const selectionBox = (await selection.boundingBox())!;
        const lineBox = (await line.boundingBox())!;
        const after = PNG.sync.read(await line.screenshot({ animations: 'disabled', caret: 'hide' }));
        expect(after.width).toBe(before.width);
        expect(after.height).toBe(before.height);

        // A blank strip at the top of the selected text excludes glyphs and the caret.
        const y = Math.max(0, Math.round(selectionBox.y - lineBox.y + 1));
        const start = Math.max(0, Math.ceil(selectionBox.x - lineBox.x + 2));
        const end = Math.min(after.width, Math.floor(selectionBox.x - lineBox.x + selectionBox.width - 2));
        let changed = 0;
        for (let x = start; x < end; x++) {
          const at = (y * after.width + x) * 4;
          if ([0, 1, 2].some((channel) => Math.abs(after.data[at + channel] - before.data[at + channel]) > 8))
            changed++;
        }
        expect(end - start).toBeGreaterThan(100);
        expect(changed / (end - start)).toBeGreaterThan(0.9);
      });
    }
  });
}

// Both viewers share the toolbar, but load documents through different app shells.
export function selectionToolbarTests(load: (page: Page, markdown: string) => Promise<void>) {
  for (const theme of ['dark', 'light']) {
    for (const width of [1280, 412]) {
      test(`selection toolbar fits its visible actions (${theme}, ${width}px)`, async ({ page }) => {
        await page.setViewportSize({ width, height: 900 });
        await load(page, markdown);
        await page.evaluate((value) => document.documentElement.setAttribute('data-theme', value), theme);
        const bar = page.getByRole('toolbar', { name: 'Format selection' });
        await expect(bar).toBeHidden();

        const cases = [
          { start: 'pre code.language-text', buttons: 1 },
          { start: 'pre code.language-js', buttons: 1 },
          { start: 'pre:not([data-lang]) code', buttons: 1 },
          { start: 'table', end: '.su-block:last-child p', buttons: 1 },
          { start: '.su-block:first-child p', end: '.su-block:last-child p', buttons: 1 },
          { start: 'td', buttons: 6 },
          { start: '.su-block:first-child p', buttons: 6 },
          // Shrink again after showing the full toolbar, without stale width or gaps.
          { start: 'pre code.language-text', buttons: 1 },
        ];
        for (const { start, end = start, buttons } of cases) {
          await test.step(`select ${start}${end !== start ? ` through ${end}` : ''}`, async () => {
            await page.locator(`#previewArticle ${start}`).first().scrollIntoViewIfNeeded();
            await page.evaluate(
              ({ start, end }) => {
                const article = document.querySelector('#previewArticle')!;
                const from = article.querySelector(start)!;
                const to = article.querySelector(end)!;
                const range = document.createRange();
                range.setStart(from, 0);
                range.setEnd(to, to.childNodes.length);
                getSelection()!.removeAllRanges();
                getSelection()!.addRange(range);
              },
              { start, end },
            );
            await expect(bar).toBeVisible();
            await expect(bar.getByRole('button')).toHaveCount(buttons);
            const box = (await bar.boundingBox())!;
            const actions = await bar.getByRole('button').all();
            const first = (await actions[0].boundingBox())!;
            const last = (await actions[actions.length - 1].boundingBox())!;
            // Four pixels of padding and one border on each side, not menu minimum width.
            expect(box.width).toBeCloseTo(last.x + last.width - first.x + 10, 0);
            expect(first.x - box.x).toBeCloseTo(5, 0);
            expect(box.x).toBeGreaterThanOrEqual(8);
            expect(box.x + box.width).toBeLessThanOrEqual(width - 8);

            await page.evaluate(() => getSelection()!.removeAllRanges());
            await expect(bar).toBeHidden();
          });
        }

        // The compact Comment action still opens the composer and hides the toolbar.
        await page.locator('#previewArticle pre code.language-text').evaluate((element) => {
          const range = document.createRange();
          range.selectNodeContents(element);
          getSelection()!.addRange(range);
        });
        await bar.getByRole('button', { name: 'Comment', exact: true }).click();
        await expect(page.locator('.cm-card.draft').getByRole('textbox', { name: 'Comment' })).toBeVisible();
        await expect(bar).toBeHidden();
      });
    }
  }
}

export function listItemTests(load: (page: Page, markdown: string) => Promise<void>) {
  const source = (page: Page) =>
    page.evaluate(() => (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText());

  for (const { marker, loose } of [
    { marker: '1.', loose: true },
    { marker: '12)', loose: false },
    { marker: '-', loose: true },
    { marker: '*', loose: false },
  ]) {
    test(`selection formatting preserves wrapped list source (${marker}, ${loose ? 'loose' : 'tight'})`, async ({
      page,
    }) => {
      const indent = ' '.repeat(marker.length + 1);
      const text = [
        '**Review item.** Updated the comment in `config.ts`.',
        'The minimum stays one minute and the default stays one hour.',
        'The temporary token has a different maximum.',
        'Checked `TokenConfig.Access` and `application.properties` against the current revision.',
        'The regression cases cover both limits.',
      ].join(`\n${indent}`);
      const item = `${marker} ${text}`;
      const md = `Plain words here.\n\n${item}\n${loose ? '\n' : ''}${marker} Keep this item.\n`;
      await load(page, md);
      const first = page.locator('#previewArticle li').first();
      await first.evaluate((element) => {
        const range = document.createRange();
        range.selectNodeContents(element.querySelector('p') ?? element);
        getSelection()!.removeAllRanges();
        getSelection()!.addRange(range);
      });
      const selected = await page.evaluate(() => getSelection()!.toString());
      const strike = page.locator('#formatBar').getByRole('button', { name: 'Strikethrough' });
      await strike.click();
      await expect.poll(() => source(page)).toBe(md.replace(item, `${marker} ~~${text}~~`));
      await expect(first.locator('del')).toHaveCount(1);
      await expect(first.locator('strong')).toHaveText('Review item.');
      await expect(first.locator('code')).toHaveCount(3);
      await expect.poll(() => page.evaluate(() => getSelection()!.toString())).toBe(selected);
      await strike.click();
      await expect.poll(() => source(page)).toBe(md);

      // A selection in a continuation line changes only that line, not its indentation.
      await first.evaluate((element) => {
        const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
        for (let node; (node = walker.nextNode()); ) {
          const from = node.textContent!.indexOf('regression cases');
          if (from < 0) continue;
          getSelection()!.setBaseAndExtent(node, from, node, from + 'regression cases'.length);
          break;
        }
      });
      await strike.click();
      await expect.poll(() => source(page)).toBe(md.replace('regression cases', '~~regression cases~~'));
      await strike.click();
      await expect.poll(() => source(page)).toBe(md);
    });
  }

  test('selection formatting skips list markers and task state', async ({ page }) => {
    const md = 'Plain words here.\n\n1. 1st item.\n2. [x] x marks the spot.\n';
    await load(page, md);
    let expected = md;
    for (const [index, text] of ['1st item.', 'x marks the spot.'].entries()) {
      const item = page.locator('#previewArticle li').nth(index);
      await item.evaluate((element) => {
        const range = document.createRange();
        range.selectNodeContents(element);
        getSelection()!.removeAllRanges();
        getSelection()!.addRange(range);
      });
      const strike = page.locator('#formatBar').getByRole('button', { name: 'Strikethrough' });
      await strike.click();
      expected = expected.replace(text, `~~${text}~~`);
      await expect.poll(() => source(page)).toBe(expected);
      await expect(item.locator('del')).toHaveText(text);
    }
    await expect(page.locator('#previewArticle').getByRole('checkbox')).toBeChecked();
  });

  for (const { name, item, next } of [
    {
      name: 'numbered',
      item: '1. **Review item.** Read `config.ts`.\n   Keep this continuation.',
      next: '2. Next item.',
    },
    { name: 'bulleted', item: '+ Review item.\n  Keep this continuation.', next: '+ Next item.' },
    { name: 'nested', item: '- Parent item.\n  - Child item.', next: '- Next item.' },
  ]) {
    test(`regular ${name} list items have no Mark done action or automatic checkbox`, async ({ page }) => {
      const md = `Plain words here.\n\n${item}\n\n${next}\n`;
      await load(page, md);
      const first = page.locator('#previewArticle li').first();
      await first.hover({ position: { x: 40, y: 10 } });
      await page.locator('#blockGripBtn').click();
      await expect(page.getByRole('menuitem', { name: 'Edit', exact: true })).toBeVisible();
      await expect(page.getByRole('menuitem', { name: /^Mark (?:not )?done$/ })).toHaveCount(0);
      await page.keyboard.press('Escape');
      await expect.poll(() => source(page)).toBe(md);
      await expect(first.getByRole('checkbox')).toHaveCount(0);
    });
  }

  test('existing task checkboxes retain Mark done and Mark not done', async ({ page }) => {
    const md = 'Plain words here.\n\n- [ ] Existing task.\n';
    await load(page, md);
    const item = page.locator('#previewArticle li').first();
    for (const [action, expected] of [
      ['Mark done', md.replace('[ ]', '[x]')],
      ['Mark not done', md],
    ]) {
      await item.hover();
      await page.locator('#blockGripBtn').click();
      await page.getByRole('menuitem', { name: action, exact: true }).click();
      await expect.poll(() => source(page)).toBe(expected);
    }
    await expect(item.getByRole('checkbox')).not.toBeChecked();
  });
}
