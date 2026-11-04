import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

export const LINK_MARKDOWN = [
  '# Preview links',
  '',
  '[Markdown site](https://example.test/markdown)',
  '',
  'https://example.test/autolink',
  '',
  '[Reference site][reference]',
  '',
  '[Relative site](/preview-target)',
  '',
  '[Protocol-relative site](//example.test/protocol-relative)',
  '',
  '<a href="https://example.test/html" target="_self" rel="nofollow opener" title="Original title">HTML <strong>site</strong></a>',
  '',
  '[Jump to destination](#destination)',
  '',
  '<div style="height:900px"></div>',
  '',
  '## Destination',
  '',
  'The review stays open.',
  '',
  '[reference]: https://example.test/reference',
].join('\n');

export function previewLinkTests(open: (page: Page, markdown: string) => Promise<void>) {
  test('preview links open new tabs without replacing the review', async ({ page, context }) => {
    await context.route(/^https?:\/\/example\.test\//, (route) =>
      route.fulfill({ contentType: 'text/html', body: '<title>Linked page</title><p>Linked page</p>' }),
    );
    await context.route('**/preview-target', (route) =>
      route.fulfill({ contentType: 'text/html', body: '<title>Relative page</title><p>Relative page</p>' }),
    );
    await open(page, LINK_MARKDOWN);
    const article = page.locator('#previewArticle');
    const originalPath = new URL(page.url()).pathname;
    const links = [
      ['Markdown site', 'https://example.test/markdown'],
      ['https://example.test/autolink', 'https://example.test/autolink'],
      ['Reference site', 'https://example.test/reference'],
      ['Relative site', new URL('/preview-target', page.url()).href],
      ['Protocol-relative site', new URL('//example.test/protocol-relative', page.url()).href],
      ['HTML site', 'https://example.test/html'],
    ];
    for (const [name, url] of links) {
      const link = article.getByRole('link', { name, exact: true });
      await expect(link).toHaveAttribute('target', '_blank');
      await expect(link).toHaveAttribute('rel', /\bnoopener\b/);
      await expect(link).toHaveAttribute('rel', /\bnoreferrer\b/);
      const popupPromise = page.waitForEvent('popup');
      // Native links retain keyboard activation as well as mouse activation.
      if (name === 'Reference site') await link.press('Enter');
      else await link.click();
      const popup = await popupPromise;
      await expect(popup).toHaveURL(url);
      expect(await popup.evaluate(() => window.opener === null)).toBe(true);
      await popup.close();
      expect(new URL(page.url()).pathname).toBe(originalPath);
      await expect(article.getByRole('heading', { name: /^Preview links/ })).toBeAttached();
    }
    const html = article.getByRole('link', { name: 'HTML site', exact: true });
    await expect(html).toHaveAttribute('title', 'Original title');
    await expect(html).toHaveAttribute('rel', /\bnofollow\b/);
    await expect(html).not.toHaveAttribute('rel', /\bopener\b/);
    // Markdown Preview saves its share state to the fragment asynchronously.
    await expect(page).toHaveURL(/#.+$/);
    const beforeAnchor = page.url();
    const jump = article.getByRole('link', { name: 'Jump to destination', exact: true });
    await expect(jump).not.toHaveAttribute('target', '_blank');
    await jump.click();
    await expect(article.getByRole('heading', { name: /^Destination/ })).toBeInViewport();
    expect(page.url()).toBe(beforeAnchor);
    expect(context.pages()).toHaveLength(1);
    const source = await page.evaluate(() =>
      (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText(),
    );
    expect(source).toBe(LINK_MARKDOWN);
  });
}
