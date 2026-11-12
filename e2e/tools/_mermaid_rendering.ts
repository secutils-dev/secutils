// Shared regression cases for the two viewers. Keep fixture data synthetic: private reports
// are reproduced locally, never committed as test fixtures or sent to the deployed hosts.
import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

export const GANTT_DOCUMENT = [
  '# Incident timeline',
  '',
  '```mermaid',
  'gantt',
  '  dateFormat HH:mm:ss',
  '  axisFormat %H:%M',
  '  section Activations',
  '  slow activations :crit, a1, 13:08:41, 14s',
  '  section SLO',
  '  slice 13:00 BAD (p99 13,448) :s1, 13:00:00, 10m',
  '  slice 13:10 BAD (p99 10,156) :s2, 13:10:00, 10m',
  '  alert fires :milestone, 13:23:51, 0s',
  '  slice 14:10 BAD (p99 13,842) :s3, 14:10:00, 10m',
  '```',
].join('\n');

/** Registers rendering cases using the host's document-opening workflow. */
export function mermaidRenderingTests(open: (page: Page, markdown: string) => Promise<void>) {
  test('Mermaid rendering: Gantt clock labels retain their text, timing, and original source', async ({ page }) => {
    await open(page, GANTT_DOCUMENT);
    const svg = page.locator('#previewArticle figure.su-mermaid .su-mermaid-stage > svg');
    await expect(svg).toBeVisible({ timeout: 20000 });
    for (const label of [
      'slice 13:00 BAD (p99 13,448)',
      'slice 13:10 BAD (p99 10,156)',
      'slice 14:10 BAD (p99 13,842)',
    ]) {
      await expect(svg).toContainText(label);
    }
    await expect(svg.locator('rect.task')).toHaveCount(5);
    const task = (id: string) => svg.locator(`rect[id$="-${id}"]`);
    const [first, second, third] = await Promise.all(['s1', 's2', 's3'].map((id) => task(id).getAttribute('x')));
    expect(Number(second)).toBeGreaterThan(Number(first));
    expect(Number(third)).toBeGreaterThan(Number(second));
    const widths = await Promise.all(['s1', 's2', 's3'].map((id) => task(id).getAttribute('width')));
    // Mermaid rounds the start and end pixels independently, so equal durations can differ by one pixel.
    expect(Number(widths[0])).toBeGreaterThan(0);
    for (const width of widths.slice(1)) expect(Math.abs(Number(width) - Number(widths[0]))).toBeLessThanOrEqual(1);
    await expect(page.locator('#previewArticle .su-mermaid-error')).toHaveCount(0);
    await expect(page.locator('body > [id^="dsu-mermaid-"]')).toHaveCount(0);
    await page.getByRole('button', { name: 'Source', exact: true }).click();
    expect(
      await page.evaluate(() =>
        (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText(),
      ),
    ).toBe(GANTT_DOCUMENT);
  });

  test('Mermaid rendering: renderer failures stay inline without stray SVGs and recover after editing', async ({
    page,
  }) => {
    // The grammar accepts four fields, but Gantt's task compiler cannot process them.
    const invalid =
      '# Invalid timeline\n\n```mermaid\ngantt\n dateFormat HH:mm:ss\n Bad task :id, 13:00:00, 10m, extra\n```';
    await open(page, invalid);
    await expect(page.locator('#previewArticle .su-mermaid-error')).toBeVisible({ timeout: 20000 });
    await expect(page.locator('#previewArticle .su-mermaid-error-src')).toContainText(
      'Bad task :id, 13:00:00, 10m, extra',
    );
    await expect(page.locator('body > [id^="dsu-mermaid-"]')).toHaveCount(0);
    await page.getByRole('button', { name: 'Source', exact: true }).click();
    await page.evaluate((markdown) => {
      (window as unknown as { __suEditorAPI: { setText(text: string): void } }).__suEditorAPI.setText(markdown);
    }, GANTT_DOCUMENT);
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    await expect(page.locator('#previewArticle figure.su-mermaid .su-mermaid-stage > svg')).toBeVisible({
      timeout: 20000,
    });
    await expect(page.locator('#previewArticle .su-mermaid-error')).toHaveCount(0);
    await expect(page.locator('body > [id^="dsu-mermaid-"]')).toHaveCount(0);
  });
}
