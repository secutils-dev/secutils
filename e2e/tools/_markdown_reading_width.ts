import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

export function readingWidthTests(load: (page: Page, markdown: string) => Promise<void>) {
  for (const width of ['full', '1100', 'custom']) {
    test(`reading width: ${width} matches the layout after reload`, async ({ page }) => {
      await load(page, '# Reading layout\n\nA paragraph for the saved reading width.');
      await expect(page.locator('#previewArticle h1')).toContainText('Reading layout');
      const options = page.getByRole('button', { name: /^(Document options|Options)$/ });
      await options.click();
      await page.getByLabel('Reading width', { exact: true }).selectOption(width);
      if (width === 'custom') await page.getByLabel('Custom width (px)', { exact: true }).fill('980');
      const maxWidth = width === 'full' ? 'none' : width === 'custom' ? '980px' : '1100px';
      await expect(page.locator('#previewArticle')).toHaveCSS('max-width', maxWidth);
      await options.click();
      // Markdown Preview saves its document in the fragment; MARS uses a document ID.
      await expect(page).toHaveURL(/#.+/);
      await page.reload();
      await expect(page.locator('#previewArticle h1')).toContainText('Reading layout');
      await options.click();
      await expect(page.getByLabel('Reading width', { exact: true })).toHaveValue(width);
      await expect(page.locator('#previewArticle')).toHaveCSS('max-width', maxWidth);
      if (width === 'custom') await expect(page.getByLabel('Custom width (px)', { exact: true })).toHaveValue('980');
      const track = await page.evaluate(() => document.documentElement.style.getPropertyValue('--su-reading-track'));
      expect(track).toBe(width === 'full' ? '1fr' : maxWidth);
      await options.click();
      const other = await page.context().newPage();
      await other.goto(page.url());
      await expect(other.locator('#previewArticle h1')).toContainText('Reading layout');
      await expect(other.locator('#previewArticle')).toHaveCSS('max-width', maxWidth);
      await other.close();
    });
  }

  test('reading width: full-width comment rails survive reload', async ({ page }) => {
    const threads = [
      {
        id: 'abcdef01',
        quote: 'A paragraph with a comment.',
        comments: [{ id: 'abcdef02', author: 'Reviewer', at: '2026-11-04T10:00:00.000Z', body: 'Margin note.' }],
      },
    ];
    await load(page, '# Setup\n\nOpen a different document.');
    await page.locator('#fileInput').setInputFiles({
      name: 'Saved width comments.md',
      mimeType: 'text/markdown',
      buffer: Buffer.from(
        `# Saved width comments\n\nA paragraph with a comment.\n\n<!-- su-comments\n${JSON.stringify(threads)}\n-->\n`,
      ),
    });
    await expect(page.locator('.cm-card')).toBeVisible();
    const options = page.getByRole('button', { name: /^(Document options|Options)$/ });
    await options.click();
    await page.getByLabel('Reading width', { exact: true }).selectOption('full');
    await options.click();
    await expect(page).toHaveURL(/#.+/);
    await page.reload();
    await expect(page.locator('.cm-card')).toBeVisible();
    await expect(page.locator('#previewArticle')).toHaveCSS('max-width', 'none');
    const layout = await page.locator('#previewScroll').evaluate((scroll) => {
      const style = getComputedStyle(scroll);
      const columns = style.gridTemplateColumns.split(' ').map(parseFloat);
      return {
        columns,
        available: scroll.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight),
        gap: parseFloat(style.columnGap),
      };
    });
    expect(layout.columns).toHaveLength(2);
    expect(Math.abs(layout.columns[0] + layout.columns[1] + layout.gap - layout.available)).toBeLessThan(2);
  });
}
