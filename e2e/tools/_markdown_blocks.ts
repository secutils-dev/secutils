import type { Locator, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

const source = (page: Page) =>
  page.evaluate(() => (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText());

async function gutterHover(page: Page, block: Locator) {
  await block.scrollIntoViewIfNeeded();
  const rect = (await block.boundingBox())!;
  const article = (await page.locator('#previewArticle').boundingBox())!;
  const scroll = (await page.locator('#previewScroll').boundingBox())!;
  const y = Math.max(scroll.y + 10, Math.min(rect.y + 12, scroll.y + scroll.height - 12));
  await page.mouse.move(article.x - 12, y);
  await expect(page.locator('#blockHandle')).toBeVisible();
  return { rect, article, scroll };
}

async function dragItem(page: Page, item: Locator, target: Locator, after: boolean) {
  await item.hover({ position: { x: 40, y: 10 } });
  const grip = page.locator('#blockGripBtn');
  const handle = (await grip.boundingBox())!;
  const end = (await target.boundingBox())!;
  await page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
  await page.mouse.down();
  await page.mouse.move(handle.x + 35, after ? end.y + end.height - 2 : end.y + 2, { steps: 12 });
  await page.mouse.up();
  await expect(page.locator('#blockDropLine')).toBeHidden();
}

export function blockInteractionTests(load: (page: Page, md: string) => Promise<void>) {
  test('block controls: gutter hover never overlaps text or numbered markers', async ({ page }) => {
    await load(
      page,
      '# Controls\n\nShort paragraph.\n\n22. A numbered review item.\n23. Next item.\n\nSeparate list.\n\n999999990. Wide marker.\n',
    );
    for (const width of [1280, 390, 320]) {
      await page.setViewportSize({ width, height: 800 });
      const sidebar = page.getByRole('button', { name: 'Toggle documents sidebar' });
      if ((await sidebar.count()) && (await sidebar.getAttribute('aria-expanded')) === 'true') await sidebar.click();
      for (const theme of ['dark', 'light']) {
        await page.evaluate((value) => (document.documentElement.dataset.theme = value), theme);
        for (const reading of ['800', 'full']) {
          const options = page.getByRole('button', { name: /^(Document options|Options)$/ });
          await options.click();
          await page.locator('#optReadingWidth').selectOption(reading);
          await options.click();
          for (const block of [
            page.locator('#previewArticle h1'),
            page.locator('#previewArticle').getByText('Short paragraph.', { exact: true }),
            page.locator('#previewArticle li').first(),
            page.locator('#previewArticle li').last(),
          ]) {
            const { rect, scroll } = await gutterHover(page, block);
            const handles = (await page.locator('#blockHandle').boundingBox())!;
            const left = await block.evaluate(
              (el) => (el.matches('li') ? el.parentElement! : el.closest('.su-block')!).getBoundingClientRect().left,
            );
            expect(handles.x).toBeGreaterThanOrEqual(scroll.x);
            expect(handles.x + handles.width).toBeLessThanOrEqual(left - 6);
            expect(handles.y).toBeGreaterThanOrEqual(scroll.y);
            expect(handles.y + handles.height).toBeLessThanOrEqual(scroll.y + scroll.height);
            await expect(page.locator('#blockAddBtn')).toBeVisible();
            await expect(page.locator('#blockGripBtn')).toBeVisible();
            expect(rect.width).toBeGreaterThan(0);
          }
        }
      }
    }
  });

  test('block controls: left whitespace selects the correct list item and add action', async ({ page }) => {
    const md = '# Controls\n\n- First item.\n- Wrapped item.\n  Keep this continuation.\n- Last item.\n';
    await load(page, md);
    const item = page.locator('#previewArticle li').filter({ hasText: 'Wrapped item.' });
    await gutterHover(page, item);
    await page.locator('#blockGripBtn').click();
    await page.getByRole('menuitem', { name: 'Edit', exact: true }).click();
    await expect(page.locator('.su-block-editor')).toBeVisible();
    await page.keyboard.press('Escape');
    await expect.poll(() => source(page)).toBe(md);
    await gutterHover(page, item);
    await page.locator('#blockAddBtn').click();
    await page.keyboard.type('Inserted item.');
    await page.keyboard.press('Escape');
    await expect.poll(() => source(page)).toBe(md.replace('- Last item.', '- Inserted item.\n- Last item.'));
  });

  for (const nested of [false, true]) {
    test(`block controls: ${nested ? 'nested' : 'ordered'} list gaps retain adjacent item controls`, async ({
      page,
    }) => {
      const second = nested ? '   - Second item.' : '2. Second item.';
      const md = nested
        ? `# Review\n\n1. Parent\n\n   - First item.  \n     Continuation.\n\n${second}\n\n   - Third item.\n\n2. Outside.\n`
        : `# Review\n\n1. First item.  \n   Continuation.\n\n${second}\n\n3. Third item.\n`;
      await load(page, md);
      const items = page.locator(nested ? '#previewArticle ol ul > li' : '#previewArticle > .su-block > ol > li');
      await expect(items).toHaveCount(3);
      await gutterHover(page, items.nth(1));
      const before = (await page.locator('#blockHandle').boundingBox())!;
      const two = (await items.nth(1).boundingBox())!;
      const three = (await items.nth(2).boundingBox())!;
      const article = (await page.locator('#previewArticle').boundingBox())!;
      const gap = three.y - (two.y + two.height);
      expect(gap).toBeGreaterThan(0);
      for (const part of [0.1, 0.5, 0.9]) {
        await page.mouse.move(article.x - 12, two.y + two.height + gap * part);
        expect(Math.abs((await page.locator('#blockHandle').boundingBox())!.y - before.y)).toBeLessThan(1);
      }

      // The controls must still edit item 2, rather than the entire list.
      await page.locator('#blockGripBtn').click();
      await page.getByRole('menuitem', { name: 'Edit', exact: true }).click();
      await page.keyboard.press('ControlOrMeta+a');
      await page.keyboard.type(second.replace('Second item.', 'Revised target.'));
      await page.keyboard.press('Escape');
      await expect.poll(() => source(page)).toBe(md.replace('Second item.', 'Revised target.'));
      await page.locator('#docStats').click();
      await page.keyboard.press('ControlOrMeta+z');
      await expect(items.nth(1)).toContainText('Second item.');

      // Entering the same gap from item 3 keeps item 3 selected.
      await gutterHover(page, items.nth(2));
      const reverse = (await page.locator('#blockHandle').boundingBox())!;
      await page.mouse.move(article.x - 12, two.y + two.height + gap / 2);
      expect(Math.abs((await page.locator('#blockHandle').boundingBox())!.y - reverse.y)).toBeLessThan(1);

      // A direct hover from outside the list picks an adjacent item, never item 1/parent.
      await gutterHover(page, page.locator('#previewArticle h1'));
      await page.mouse.move(article.x - 12, two.y + two.height + gap * 0.25);
      expect(Math.abs((await page.locator('#blockHandle').boundingBox())!.y - before.y)).toBeLessThan(1);
      await page.locator('#blockAddBtn').click();
      await page.keyboard.type('Inserted after second.');
      await page.keyboard.press('Escape');
      await expect(items).toHaveCount(4);
      await expect(items.nth(2)).toContainText('Inserted after second.');
      await expect(items.nth(3)).toContainText('Third item.');
    });
  }

  test('block controls: comment rails keep a separate full-width gutter', async ({ page }) => {
    await load(page, '# Setup\n\nOpen a different document.');
    const thread = [
      {
        id: 'abcdef01',
        quote: 'Review this paragraph.',
        comments: [{ id: 'abcdef02', author: 'Reviewer', at: '2026-11-04T10:00:00.000Z', body: 'Margin note.' }],
      },
    ];
    await page.locator('#fileInput').setInputFiles({
      name: 'Margin controls.md',
      mimeType: 'text/markdown',
      buffer: Buffer.from(
        `# Margin controls\n\nReview this paragraph.\n\n1. First item.\n2. Second item.\n\n<!-- su-comments\n${JSON.stringify(thread)}\n-->\n`,
      ),
    });
    await expect(page.locator('.cm-card')).toBeVisible();
    const options = page.getByRole('button', { name: /^(Document options|Options)$/ });
    await options.click();
    await page.locator('#optReadingWidth').selectOption('full');
    await options.click();
    await gutterHover(page, page.locator('#previewArticle li').first());
    const handle = (await page.locator('#blockHandle').boundingBox())!;
    const article = (await page.locator('#previewArticle').boundingBox())!;
    const rail = (await page.locator('#commentsRail').boundingBox())!;
    expect(handle.x + handle.width).toBeLessThan(article.x);
    expect(article.x + article.width).toBeLessThan(rail.x);
  });

  test('list dragging: nested multiline items move in both directions and ordered sources renumber', async ({
    page,
  }) => {
    const md = '# Lists\n\n1. Parent\n   1. Short\n   2. Wrapped  \n      continuation\n   3. Last\n2. Outside\n';
    await load(page, md);
    const items = page.locator('#previewArticle ol ol > li');
    await expect(items).toHaveCount(3);
    await expect(items.nth(1)).toHaveAttribute('data-start', /\d+/);
    await dragItem(page, items.first(), items.nth(1), true);
    const down = md.replace(
      '   1. Short\n   2. Wrapped  \n      continuation\n',
      '   1. Wrapped  \n      continuation\n   2. Short\n',
    );
    await expect.poll(() => source(page)).toBe(down);
    await expect(items.first()).toContainText('Wrapped');
    await expect(items.nth(1)).toHaveText('Short');
    await dragItem(page, items.last(), items.first(), false);
    const up = md.replace(
      '   1. Short\n   2. Wrapped  \n      continuation\n   3. Last',
      '   1. Last\n   2. Wrapped  \n      continuation\n   3. Short',
    );
    await expect.poll(() => source(page)).toBe(up);
    await expect(items.first()).toHaveText('Last');
    await page.locator('#docStats').click();
    await page.keyboard.press('ControlOrMeta+z');
    await expect.poll(() => source(page)).toBe(down);
    await page.keyboard.press('ControlOrMeta+z');
    await expect.poll(() => source(page)).toBe(md);
  });

  test('list dragging: renumbering across digit widths preserves paragraphs, nested lists, and literal numbers', async ({
    page,
  }) => {
    const md =
      '# Ordered\n\n9. Short\n\n10. Wrapped\n    continuation\n\n    Second paragraph.\n\n    - child\n\n    ```md\n    42. literal\n    ```\n\n11. Last\n';
    await load(page, md);
    const items = page.locator('#previewArticle > .su-block > ol > li');
    await dragItem(page, items.nth(1), items.first(), false);
    const expected =
      '# Ordered\n\n9. Wrapped\n   continuation\n\n   Second paragraph.\n\n   - child\n\n   ```md\n   42. literal\n   ```\n\n10. Short\n\n11. Last\n';
    await expect.poll(() => source(page)).toBe(expected);
    await expect(page.locator('#previewArticle pre code')).toContainText('42. literal');
    await expect(page.locator('#previewArticle ul > li')).toHaveText('child');
  });

  test('list dragging: nested bulleted continuations are movable without renumbering', async ({ page }) => {
    const md = '# Bullets\n\n- Parent\n  - Short\n  - Wrapped  \n    continuation\n  - Last\n- Outside\n';
    await load(page, md);
    const items = page.locator('#previewArticle ul ul > li');
    await expect(items.nth(1)).toHaveAttribute('data-start', /\d+/);
    await dragItem(page, items.last(), items.nth(1), false);
    const expected = md.replace(
      '  - Wrapped  \n    continuation\n  - Last',
      '  - Last\n  - Wrapped  \n    continuation',
    );
    await expect.poll(() => source(page)).toBe(expected);
  });

  test('list dragging: keyboard moves retain the ordered start and parenthesis markers', async ({ page }) => {
    const md = '# Numbered\n\n12) Short\n13) Wrapped  \n    continuation\n14) Last\n';
    await load(page, md);
    const items = page.locator('#previewArticle > .su-block > ol > li');
    await gutterHover(page, items.nth(1));
    await page.locator('#blockGripBtn').click();
    await page.getByRole('menuitem', { name: 'Move up', exact: true }).click();
    await expect.poll(() => source(page)).toBe('# Numbered\n\n12) Wrapped  \n    continuation\n13) Short\n14) Last\n');
  });

  test('list dragging: holding at an edge scrolls past a tall multiline item', async ({ page }) => {
    const tall = Array.from({ length: 70 }, (_, i) => `  continuation line ${i}  `).join('\n');
    const md = `# Tall list\n\n- Short\n- Tall\n${tall}\n- Last\n`;
    await load(page, md);
    const items = page.locator('#previewArticle > .su-block > ul > li');
    await items.first().hover({ position: { x: 40, y: 10 } });
    const grip = (await page.locator('#blockGripBtn').boundingBox())!;
    const scroll = (await page.locator('#previewScroll').boundingBox())!;
    await page.mouse.move(grip.x + grip.width / 2, grip.y + grip.height / 2);
    await page.mouse.down();
    await page.mouse.move(grip.x + 35, Math.min(scroll.y + scroll.height, 800) - 5, { steps: 10 });
    await expect
      .poll(
        () =>
          page
            .locator('#previewScroll')
            .evaluate((el) => Math.max(el.scrollTop, document.scrollingElement?.scrollTop ?? 0)),
        { timeout: 10000 },
      )
      .toBeGreaterThan(900);
    await page.mouse.up();
    await expect.poll(() => source(page)).not.toBe(md);
    await expect(items.first()).toContainText('Tall');
    await expect(page.locator('#blockDropLine')).toBeHidden();
  });
}
