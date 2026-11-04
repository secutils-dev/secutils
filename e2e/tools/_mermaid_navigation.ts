import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

const DIAGRAM = [
  '# Diagrams',
  '',
  '```mermaid',
  'flowchart LR',
  ...Array.from({ length: 12 }, (_, i) => `N${i}[Step ${i}] --> N${i + 1}[Step ${i + 1}]`),
  `N0["Start<br/>${Array.from({ length: 30 }, () => 'Detail').join('<br/>')}"]`,
  '```',
  '',
  '```mermaid',
  'flowchart TD',
  'A[Other diagram] --> B[End]',
  '```',
].join('\n');

const FIRST_DIAGRAM = DIAGRAM.match(/```mermaid\n[\s\S]*?\n```/)![0];
const STATE_DOCUMENT = `${DIAGRAM.replace('# Diagrams', '# Diagram state\n\nUnrelated paragraph.')}\n\n${FIRST_DIAGRAM}`;

const views = (page: Page) =>
  page.locator('#previewArticle figure.su-mermaid').evaluateAll((figures) =>
    figures.map((fig) => {
      const viewport = fig.querySelector('.su-mermaid-viewport')!;
      return {
        zoom: fig.querySelector('.su-mermaid-zoom')!.textContent,
        left: viewport.scrollLeft,
        top: viewport.scrollTop,
      };
    }),
  );

async function zoomAndPan(page: Page) {
  const figures = page.locator('#previewArticle figure.su-mermaid');
  await expect(figures).toHaveCount(3);
  for (let i = 0; i < 3; i++) {
    const fig = figures.nth(i);
    await fig.getByRole('button', { name: /Show actual size/ }).click();
    for (let n = 0; n <= i; n++) await fig.getByRole('button', { name: 'Zoom in', exact: true }).click();
    await fig.locator('.su-mermaid-viewport').evaluate((viewport, index) => {
      viewport.scrollLeft = 300 + index * 50;
      viewport.scrollTop = 150 + index * 40;
    }, i);
  }
  return views(page);
}

async function sourceEdit(page: Page, text: string) {
  await page.evaluate((md) => {
    (window as unknown as { __suEditorAPI: { setText(t: string): void } }).__suEditorAPI.setText(md);
  }, text);
}

export function mermaidNavigationTests(open: (page: Page, markdown: string) => Promise<void>) {
  test('Mermaid navigation: unrelated block and Source edits retain independent views', async ({ page }) => {
    await open(page, STATE_DOCUMENT);
    const before = await zoomAndPan(page);
    const article = page.locator('#previewArticle');
    await article.getByText('Unrelated paragraph.', { exact: true }).dblclick();
    await expect(page.locator('.su-block-editor')).toBeVisible();
    await page.keyboard.press('ControlOrMeta+a');
    await page.keyboard.type('Edited unrelated **paragraph**.');
    await page.keyboard.press('Escape');
    await expect(article.locator('strong')).toHaveText('paragraph');
    await expect.poll(() => views(page)).toEqual(before);

    // Hiding the preview must retain pan even when several renders happen while hidden.
    await page.getByRole('button', { name: 'Source', exact: true }).click();
    for (const paragraph of ['First Source edit.', 'Second Source edit.']) {
      await sourceEdit(page, STATE_DOCUMENT.replace('Unrelated paragraph.', paragraph));
      await expect(article.locator(':scope > .su-block > p').filter({ hasText: paragraph })).toHaveText(paragraph);
    }
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    await expect.poll(() => views(page)).toEqual(before);
    await page.getByRole('button', { name: 'Source', exact: true }).click();
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    await expect.poll(() => views(page)).toEqual(before);
  });

  test('Mermaid navigation: changed diagrams and newly opened documents reset only their own views', async ({
    page,
  }) => {
    await open(page, STATE_DOCUMENT);
    const before = await zoomAndPan(page);
    await page.getByRole('button', { name: 'Source', exact: true }).click();
    await sourceEdit(page, STATE_DOCUMENT.replace('N0["Start<br/>', 'N0["Changed start<br/>'));
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    const figures = page.locator('#previewArticle figure.su-mermaid');
    await expect(figures.first()).toContainText('Changed start');
    await expect.poll(async () => (await views(page)).slice(1)).toEqual(before.slice(1));
    await expect.poll(async () => (await views(page))[0].zoom).not.toBe(before[0].zoom);
    expect((await views(page))[0]).toMatchObject({ left: 0, top: 0 });

    // The unchanged duplicate must still obey Fit after restoration.
    await figures.last().getByRole('button', { name: 'Fit diagram', exact: true }).click();
    await sourceEdit(
      page,
      STATE_DOCUMENT.replace('N0["Start<br/>', 'N0["Changed start<br/>').replace(
        'Unrelated paragraph.',
        'Another edit.',
      ),
    );
    await expect(page.locator('#previewArticle').getByText('Another edit.', { exact: true })).toBeVisible();
    await expect.poll(async () => (await views(page))[2].left).toBe(0);
    await expect.poll(async () => (await views(page))[2].top).toBe(0);

    await page.locator('#fileInput').setInputFiles({
      name: 'Another document.md',
      mimeType: 'text/markdown',
      buffer: Buffer.from(STATE_DOCUMENT.replace('# Diagram state', '# Another document')),
    });
    // MARS recognizes the shared diagram source as a possible updated copy.
    if (await page.getByRole('tree', { name: 'Documents', exact: true }).count()) {
      await page.getByRole('button', { name: 'Add as new', exact: true }).click();
    }
    await expect(page.locator('#previewArticle h1')).toContainText('Another document');
    await expect
      .poll(() =>
        figures.evaluateAll((items) =>
          items.every(
            (fig) =>
              (fig as HTMLElement & { __suReadMermaidView: () => { fitting: boolean } }).__suReadMermaidView().fitting,
          ),
        ),
      )
      .toBe(true);
    expect((await views(page)).map(({ left, top }) => ({ left, top }))).toEqual([
      { left: 0, top: 0 },
      { left: 0, top: 0 },
      { left: 0, top: 0 },
    ]);
  });

  test('Mermaid navigation: zoom, pan, fit, fullscreen, and independent diagrams', async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await open(page, DIAGRAM);
    const figures = page.locator('#previewArticle figure.su-mermaid');
    await expect(figures).toHaveCount(2);
    const fig = figures.first();
    const view = fig.getByRole('region', { name: /^Mermaid diagram/ });
    const percent = fig.getByRole('button', { name: /Show actual size/ });
    await expect(percent).toHaveText(/%$/);
    const initial = await percent.textContent();
    const otherInitial = await figures
      .last()
      .getByRole('button', { name: /Show actual size/ })
      .textContent();
    const scroll = () => view.evaluate((v) => ({ x: v.scrollLeft, y: v.scrollTop }));
    await fig.getByRole('button', { name: 'Zoom in', exact: true }).click();
    await expect(percent).not.toHaveText(initial!);
    await percent.click();
    await expect(percent).toHaveText('100%');
    await expect(view).toHaveCSS('overflow', 'auto');
    const bounds = (await view.boundingBox())!;
    await view.evaluate((v) => {
      v.scrollLeft = 300;
    });
    const start = await scroll();
    await page.mouse.move(bounds.x + 180, bounds.y + 70);
    await page.mouse.down();
    await page.mouse.move(bounds.x + 100, bounds.y + 40, { steps: 5 });
    await page.mouse.up();
    await expect.poll(async () => (await scroll()).x).toBeGreaterThan(start.x + 60);
    await expect.poll(async () => (await scroll()).y).toBeGreaterThan(start.y + 20);
    await expect(view).not.toHaveClass(/su-panning/);
    await view.press('ArrowRight');
    await expect.poll(async () => (await scroll()).x).toBeGreaterThan(start.x + 80);

    // Trackpad pinch and Ctrl/⌘ + wheel zoom at the pointer, without moving the page.
    const before = await view.evaluate((v) => {
      const r = v.getBoundingClientRect();
      const svg = v.querySelector('svg')!;
      const s = svg.getBoundingClientRect();
      const point = [r.left + 90, r.top + 60];
      const anchor = [(point[0] - s.left) / s.width, (point[1] - s.top) / s.height];
      const prevented = !v.dispatchEvent(
        new WheelEvent('wheel', {
          deltaY: -30,
          ctrlKey: true,
          clientX: point[0],
          clientY: point[1],
          bubbles: true,
          cancelable: true,
        }),
      );
      const after = svg.getBoundingClientRect();
      return {
        prevented,
        anchor,
        after: [(point[0] - after.left) / after.width, (point[1] - after.top) / after.height],
      };
    });
    expect(before.prevented).toBe(true);
    expect(before.after[0]).toBeCloseTo(before.anchor[0], 2);
    expect(before.after[1]).toBeCloseTo(before.anchor[1], 2);
    await expect(percent).not.toHaveText('100%');
    await fig.getByRole('button', { name: 'Fit diagram', exact: true }).click();
    await expect(percent).toHaveText(initial!);
    expect(await scroll()).toEqual({ x: 0, y: 0 });
    await view.press('+');
    await expect(percent).not.toHaveText(initial!);
    await view.press('0');
    await expect(percent).toHaveText(initial!);
    await expect(figures.last().getByRole('button', { name: /Show actual size/ })).toHaveText(otherInitial!);

    const full = fig.getByRole('button', { name: 'Toggle diagram fullscreen' });
    await full.click();
    await expect.poll(() => page.evaluate(() => document.fullscreenElement?.className)).toBe('su-mermaid');
    await expect(full).toHaveAttribute('aria-pressed', 'true');
    await expect.poll(async () => (await view.boundingBox())!.height).toBeGreaterThan(600);
    await percent.click();
    await view.press('+');
    await expect(percent).toHaveText('125%');
    await full.click();
    await expect.poll(() => page.evaluate(() => document.fullscreenElement === null)).toBe(true);
    await expect(full).toHaveAttribute('aria-pressed', 'false');
    await expect(percent).toHaveText('125%');
    expect(errors).toEqual([]);
  });

  test('Mermaid navigation: phone layout, themes, zoom limits, and rerender cleanup', async ({ page }) => {
    await open(page, DIAGRAM);
    await page.setViewportSize({ width: 390, height: 800 });
    const fig = page.locator('#previewArticle figure.su-mermaid').first();
    const view = fig.getByRole('region', { name: /^Mermaid diagram/ });
    const percent = fig.getByRole('button', { name: /Show actual size/ });
    await expect(percent).toHaveText(/%$/);
    for (const theme of ['light', 'dark']) {
      await page.evaluate((value) => document.documentElement.setAttribute('data-theme', value), theme);
      await expect(fig.getByRole('button', { name: 'Fit diagram', exact: true })).toBeVisible();
      const geometry = await fig.evaluate((f) => {
        const tools = f.querySelector('.su-mermaid-tools')!;
        return {
          width: f.clientWidth,
          toolsWidth: tools.scrollWidth,
          touch: getComputedStyle(f.querySelector('.su-mermaid-viewport')!).touchAction,
        };
      });
      expect(geometry.toolsWidth).toBeLessThanOrEqual(geometry.width);
      expect(geometry.touch).toBe('auto');
    }
    await percent.click();
    for (let i = 0; i < 10; i++) await view.press('+');
    await expect(percent).toHaveText('800%');
    await expect(fig.getByRole('button', { name: 'Zoom in', exact: true })).toBeDisabled();
    for (let i = 0; i < 40; i++) await view.press('-');
    await expect(fig.getByRole('button', { name: 'Zoom out', exact: true })).toBeDisabled();
    await fig.getByRole('button', { name: 'Fit diagram', exact: true }).click();
    const svg = fig.locator('.su-mermaid-stage > svg');
    const box = (await svg.boundingBox())!;
    const viewport = (await view.boundingBox())!;
    expect(box.width).toBeLessThanOrEqual(viewport.width);
    expect(box.height).toBeLessThanOrEqual(viewport.height);
    await page.setViewportSize({ width: 1280, height: 800 });
    await expect.poll(async () => (await svg.boundingBox())!.width).toBeGreaterThan(box.width);

    // A replaced preview must detach per-diagram observers and document listeners.
    await page.evaluate(() => {
      const fig = document.querySelector('figure.su-mermaid') as HTMLElement & { __suDispose: () => void };
      const dispose = fig.__suDispose;
      (window as unknown as { __mermaidDisposed: boolean }).__mermaidDisposed = false;
      fig.__suDispose = () => {
        dispose();
        (window as unknown as { __mermaidDisposed: boolean }).__mermaidDisposed = true;
      };
      (window as unknown as { __suEditorAPI: { setText(t: string): void } }).__suEditorAPI.setText(
        '# Replaced\n\nNo diagram.',
      );
    });
    await expect(page.locator('#previewArticle h1')).toContainText('Replaced');
    expect(await page.evaluate(() => (window as unknown as { __mermaidDisposed: boolean }).__mermaidDisposed)).toBe(
      true,
    );
    await expect(page.locator('.su-mermaid-tools')).toHaveCount(0);
  });
}
