import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

const markdown = [
  'Plain words here.',
  '```text\nfenced code words\n```',
  '```js\nconst highlighted = true;\n```',
  '    indented code words',
  '| Column | Value |\n| --- | --- |\n| table words | cell words |',
  'Last paragraph.',
].join('\n\n');

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
