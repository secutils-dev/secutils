import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import type { Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';

const tool = getTool('md');
const modelFixture = (name: string) => resolve(__dirname, `fixtures/3mf/${name}.3mf`);

// Read the actual GPU output, rather than inferring appearance from the DOM.
async function modelPixels(page: Page): Promise<{ red: number; green: number; blue: number; mixed: number }> {
  return page.locator('figure[data-kind="su-mesh"] canvas').evaluate((element) => {
    const canvas = element as HTMLCanvasElement;
    const gl = canvas.getContext('webgl')!;
    const pixels = new Uint8Array(canvas.width * canvas.height * 4);
    gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
    const counts = { red: 0, green: 0, blue: 0, mixed: 0 };
    for (let i = 0; i < pixels.length; i += 4) {
      const [r, g, b] = pixels.subarray(i, i + 3);
      if (r > 80 && r > g * 1.8 && r > b * 1.8) counts.red++;
      if (g > 80 && g > r * 1.8 && g > b * 1.8) counts.green++;
      if (b > 80 && b > r * 1.8 && b > g * 1.8) counts.blue++;
      if (r > 30 && g > 30 && b < 100 && Math.abs(r - g) < 30) counts.mixed++;
    }
    return counts;
  });
}

// Drives the imperative editor API the page exposes once the editor mounts
// (CodeMirror from esm.sh, or a <textarea> fallback). This keeps tests
// independent of the editor's internal DOM.
async function setMarkdown(page: Page, md: string): Promise<void> {
  await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
  await page.evaluate((text) => {
    (window as unknown as { __suEditorAPI: { setText(t: string): void } }).__suEditorAPI.setText(text);
  }, md);
}

test.describe(`${tool.name} (${tool.path})`, () => {
  test('SEO head block matches the AGENTS.md SEO budget', async ({ page }) => {
    const response = await page.goto(tool.path);
    expect(response?.ok()).toBeTruthy();
    await assertSeoBasics(page, tool);
  });

  test('skill .md is reachable with required frontmatter', async ({ request }) => {
    await assertSkillMd(request, tool);
  });

  test('reading-first: preview is the default view and editor is hidden', async ({ page }) => {
    await page.goto(tool.path);
    await expect(page.locator('#previewWrap')).toBeVisible();
    await expect(page.locator('#editorMount')).toBeHidden();
    // The empty start state offers actionable entry points.
    await expect(page.locator('#emptyState')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Open file' })).toBeVisible();
  });

  test('renders Markdown into the preview', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, '# Hello *world*\n\n- item 1\n- item 2');

    await expect(page.locator('#previewArticle h1')).toContainText('Hello');
    await expect(page.locator('#previewArticle ul li')).toHaveCount(2);
    await expect(page.locator('#emptyState')).toBeHidden();
  });

  test('unmatched inline HTML stays literal without swallowing the rest of the document', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(
      page,
      [
        'An ESLint/Oxlint plugin to enforce rules like "no raw colors" and',
        '"no restyling <Button> from outside" with errors to steer agents',
        'back to your variants and tokens.',
        '',
        'Unmatched <span class="example"> and </em> stay visible.',
        '',
        'Paired <em>HTML</em>, <button>Click</button>, and a break<br>still work.',
        '',
        '## Next section',
        '',
        '- A later item',
      ].join('\n'),
    );

    const preview = page.locator('#previewArticle');
    await expect(preview).toContainText('"no restyling <Button> from outside"');
    await expect(preview).toContainText('Unmatched <span class="example"> and </em> stay visible.');
    await expect(preview.locator('button')).toHaveText(['Click']);
    await expect(preview.locator('em')).toHaveText(['HTML']);
    await expect(preview.locator('br')).toHaveCount(1);
    await expect(preview.locator(':scope > .su-block > h2')).toContainText('Next section');
    await expect(preview.locator(':scope > .su-block > ul > li')).toHaveText(['A later item']);

    await page.getByRole('button', { name: 'HTML', exact: true }).click();
    const exported = page.frameLocator('#htmlPreview').locator('article');
    await expect(exported).toContainText('"no restyling <Button> from outside"');
    await expect(exported.locator('button')).toHaveText(['Click']);
    await expect(exported.locator(':scope > h2')).toContainText('Next section');
    await expect(exported.locator(':scope > ul > li')).toHaveText(['A later item']);
  });

  test('extracts export titles as text, including entities and multiline headings', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, '<h1>A &amp; B\n<em>Title</em> &lt;script&gt;</h1>');
    await page.getByRole('button', { name: 'HTML', exact: true }).click();
    const frame = page.frameLocator('#htmlPreview');
    await expect
      .poll(() => frame.locator('html').evaluate((el) => el.ownerDocument.title))
      .toBe('A & B Title <script>');
    await expect(frame.locator('article h1')).toContainText('A & B');
  });

  test('Source view swaps preview for the editor and back', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, '# Editable');

    await page.getByRole('button', { name: 'Source', exact: true }).click();
    await expect(page.locator('#editorMount')).toBeVisible();
    await expect(page.locator('#previewWrap')).toBeHidden();

    await page.keyboard.press('Escape');
    await expect(page.locator('#previewWrap')).toBeVisible();
    await expect(page.locator('#editorMount')).toBeHidden();
  });

  test('HTML view renders the exported document in an iframe', async ({ page }) => {
    await page.goto(tool.path);
    // The HTML segment is disabled until there is content.
    await expect(page.getByRole('button', { name: 'HTML', exact: true })).toBeDisabled();

    await setMarkdown(page, '# Exported\n\nBody text.');
    await page.getByRole('button', { name: 'HTML', exact: true }).click();

    const frame = page.frameLocator('#htmlPreview');
    await expect(frame.locator('article h1')).toContainText('Exported');
    await expect(page.locator('#previewWrap')).toBeHidden();

    // The "Find in page" HTML option (default on) adds an in-export find widget.
    await frame.locator('#find-btn').click();
    await expect(frame.locator('#su-find.open')).toBeVisible();
    await frame.locator('#su-find input').fill('Body');
    await expect(frame.locator('article mark.su-find-hit')).toContainText('Body');
  });

  test('reading width presets and custom value match Preview, HTML view, and export', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, '# Width test\n\nA readable paragraph.');
    const article = page.locator('#previewArticle');
    const htmlArticle = page.frameLocator('#htmlPreview').locator('article');
    await expect(article).toHaveCSS('max-width', '800px');

    await page.getByRole('button', { name: 'Options', exact: true }).click();
    const width = page.getByLabel('Reading width');
    await width.selectOption('1100');
    await expect(article).toHaveCSS('max-width', '1100px');
    await page.getByRole('button', { name: 'HTML', exact: true }).click();
    await expect(htmlArticle).toHaveCSS('max-width', '1100px');

    await page.getByRole('button', { name: 'Options', exact: true }).click();
    await width.selectOption('full');
    await expect(article).toHaveCSS('max-width', 'none');
    await expect(htmlArticle).toHaveCSS('max-width', 'none');

    await width.selectOption('custom');
    const custom = page.getByLabel('Custom width (px)');
    await expect(custom).toBeVisible();
    await custom.fill('960');
    await expect(article).toHaveCSS('max-width', '960px');
    await expect(htmlArticle).toHaveCSS('max-width', '960px');
    await custom.fill('9999');
    await custom.press('Tab');
    await expect(custom).toHaveValue('960');
    await expect(htmlArticle).toHaveCSS('max-width', '960px');

    const downloadPromise = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Export', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Download HTML' }).click();
    const output = await readFile(await (await downloadPromise).path(), 'utf8');
    expect(output).toContain('style="--su-reading-width: 960px"');
    expect(output).not.toContain('--su-reading-width: 9999px');
  });

  test('exported HTML lets readers change width without changing its default', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, '# Reader\n\n## First\n\nA\n\n## Second\n\nB\n\n## Third\n\nC');
    await page.getByRole('button', { name: 'Options', exact: true }).click();
    await page.getByLabel('Reading width').selectOption('1100');
    const downloadPromise = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Export', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Download HTML' }).click();
    const output = await readFile(await (await downloadPromise).path(), 'utf8');

    await page.route('https://example.test/exported.html', (route) =>
      route.fulfill({ contentType: 'text/html', body: output }),
    );
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto('https://example.test/exported.html');
    const article = page.locator('main article');
    await expect(article).toHaveCSS('max-width', '1100px');
    await expect(page.locator('#toc')).toBeVisible();
    await page.getByRole('button', { name: 'Reading settings' }).click();
    await page.getByLabel('Reading width').selectOption('full');
    await expect(article).toHaveCSS('max-width', 'none');
    const tocBox = await page.locator('#toc').boundingBox();
    const articleBox = await article.boundingBox();
    expect(tocBox && articleBox && articleBox.x >= tocBox.x + tocBox.width + 16).toBeTruthy();
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(1440);

    await page.setViewportSize({ width: 390, height: 844 });
    await expect(page.locator('#toc')).toBeHidden();
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
    const settingsBox = await page.locator('#reading-settings').boundingBox();
    expect(settingsBox && settingsBox.x >= 0 && settingsBox.x + settingsBox.width <= 390).toBeTruthy();

    await page.getByRole('button', { name: 'Use document default' }).click();
    await expect(article).toHaveCSS('max-width', '1100px');
    await page.getByLabel('Reading width').selectOption('custom');
    await page.getByLabel('Custom width (px)').fill('900');
    await expect(article).toHaveCSS('max-width', '900px');
    await page.reload();
    await expect(article).toHaveCSS('max-width', '1100px');

    await page.emulateMedia({ media: 'print' });
    await expect(article).toHaveCSS('max-width', 'none');
    await expect(page.locator('#reading-settings')).toBeHidden();
  });

  test('GitHub alerts and ==highlights== are enhanced', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, '> [!NOTE]\n> Heads up.\n\nSome ==marked== text.');

    await expect(page.locator('#previewArticle .markdown-alert-note')).toBeVisible();
    await expect(page.locator('#previewArticle .markdown-alert-title')).toContainText('Note');
    await expect(page.locator('#previewArticle mark.su-hl')).toContainText('marked');
  });

  test('highlights inline code in tables in preview and HTML export', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(
      page,
      [
        '| Behaviour | **Workflows** (this PR) | AB **conversation** | AB **agent** | **Dashboards** write-restricted |',
        '| --- | --- | --- | --- | --- |',
        '| Storage | own ES index | own ES index | own ES index | root metadata on the saved object |',
        '| Field name | `access_control` | `access_control` | `access_control` | `accessControl` |',
        '| Modes | `private`, `public` | `private`, `public` | `private`, `shared`, `public` | `default`, `write_restricted` |',
        '| What the restrictive mode restricts | visibility | visibility | visibility | ==writes only - everyone still reads== |',
        '| Default for new objects | ==`public`== | `private` | `private` | `default` |',
      ].join('\n'),
    );

    const preview = page.locator('#previewArticle');
    await expect(preview.locator('mark.su-hl')).toHaveText(['writes only - everyone still reads', 'public']);
    await expect(preview.getByRole('cell', { name: 'public', exact: true }).locator('mark.su-hl code')).toBeVisible();
    await expect(preview).not.toContainText('==');

    await page.getByRole('button', { name: 'HTML', exact: true }).click();
    const exported = page.frameLocator('#htmlPreview').locator('article');
    await expect(exported.locator('mark.su-hl')).toHaveText(['writes only - everyone still reads', 'public']);
    await expect(exported.getByRole('cell', { name: 'public', exact: true }).locator('mark.su-hl code')).toBeVisible();
    await expect(exported).not.toContainText('==');
  });

  test('highlights preserve nested inline formatting and literal code delimiters', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(
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
    await page.goto(tool.path);
    await setMarkdown(
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

  test('mermaid code blocks render to an inline SVG diagram', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, ['```mermaid', 'flowchart LR', '  A[Start] --> B[End]', '```'].join('\n'));

    // Mermaid is lazy-loaded from a CDN, so allow extra time for the diagram.
    await expect(page.locator('#previewArticle figure.su-mermaid svg')).toBeVisible({ timeout: 15000 });
  });

  test('GitHub maps and STL models remain interactive in offline HTML exports', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(
      page,
      `# Diagrams

\`\`\`geojson
{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"name":"Region <script>alert(1)</script>"},"geometry":{"type":"Polygon","coordinates":[[[-90,35],[-90,30],[-85,30],[-85,35],[-90,35]]]}}]}
\`\`\`

\`\`\`topojson
{"type":"Topology","transform":{"scale":[0.1,0.1],"translate":[100,0]},"objects":{"point":{"type":"Point","coordinates":[40,50]},"route":{"type":"LineString","arcs":[0]}} ,"arcs":[[[0,0],[10,10],[10,-10]]]}
\`\`\`

\`\`\`stl
solid corner
facet normal 0 0 1
outer loop
vertex 0 0 0
vertex 1 0 0
vertex 0 1 0
endloop
endfacet
facet normal 0 1 0
outer loop
vertex 0 0 0
vertex 0 0 1
vertex 1 0 0
endloop
endfacet
endsolid corner
\`\`\``,
    );
    const figures = page.locator('#previewArticle figure.su-diagram');
    await expect(figures).toHaveCount(3, { timeout: 15000 });
    const map = figures.nth(0);
    await expect(map.locator('svg g path')).toHaveCount(1);
    await expect(map.locator('svg image').first()).toHaveAttribute('href', /tile\.openstreetmap\.org/);
    await expect(map.locator('details')).toHaveCount(0);
    const initialMap = await map.locator('svg g path').getAttribute('d');
    await map.getByRole('button', { name: 'Zoom in', exact: true }).click();
    expect(await map.locator('svg g path').getAttribute('d')).not.toBe(initialMap);
    await map.getByRole('button', { name: 'Reset view' }).click();
    await expect(map.locator('svg g path')).toHaveAttribute('d', initialMap!);
    await expect(figures.nth(1).locator('svg circle')).toHaveCount(1);
    await expect(figures.nth(1).locator('svg g path')).toHaveCount(1);
    const model = figures.nth(2);
    await expect(model.locator('svg polygon')).toHaveCount(2);
    await expect(model.locator('svg polygon').first()).toHaveAttribute('stroke', 'none');
    await model.getByRole('button', { name: 'Toggle viewer fullscreen' }).click();
    await expect.poll(() => page.evaluate(() => document.fullscreenElement?.getAttribute('data-kind'))).toBe('stl');
    await model.getByRole('button', { name: 'Toggle viewer fullscreen' }).click();
    await expect.poll(() => page.evaluate(() => document.fullscreenElement === null)).toBe(true);
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
    await model.getByRole('button', { name: 'Copy source' }).click();
    expect(await page.evaluate(() => navigator.clipboard.readText())).toMatch(/^solid corner/);
    const initialModel = await model.locator('.su-diagram-view').innerHTML();
    await model.locator('.su-diagram-view').focus();
    await page.keyboard.press('ArrowRight');
    expect(await model.locator('.su-diagram-view').innerHTML()).not.toBe(initialModel);
    await model.getByRole('button', { name: 'Reset view' }).click();
    const beforePan = await model.locator('svg polygon').first().getAttribute('points');
    await model.getByRole('button', { name: 'Pan right', exact: true }).click();
    const afterPan = await model.locator('svg polygon').first().getAttribute('points');
    const points = (value: string) => value.split(' ').map((point) => point.split(',').map(Number));
    expect(points(afterPan!)).toEqual(points(beforePan!).map(([x, y]) => [x + 20, y]));
    await model.locator('.su-diagram-view').focus();
    await page.keyboard.press('Shift+ArrowLeft');
    await expect(model.locator('svg polygon').first()).toHaveAttribute('points', beforePan!);
    const viewBox = await model.locator('.su-diagram-view').boundingBox();
    await page.keyboard.down('Shift');
    await page.mouse.move(viewBox!.x + viewBox!.width / 2, viewBox!.y + viewBox!.height / 2);
    await page.mouse.down();
    await page.mouse.move(viewBox!.x + viewBox!.width / 2 + 30, viewBox!.y + viewBox!.height / 2);
    await page.mouse.up();
    await page.keyboard.up('Shift');
    expect(await model.locator('svg polygon').first().getAttribute('points')).not.toBe(beforePan);
    await model.getByRole('button', { name: 'Reset view' }).click();
    await expect(model.locator('svg polygon').first()).toHaveAttribute('points', beforePan!);
    await model.getByRole('button', { name: 'Wireframe', exact: true }).click();
    await expect(model.locator('svg polygon').first()).toHaveAttribute('fill', 'none');

    const downloadPromise = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Export', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Download HTML' }).click();
    const output = await readFile(await (await downloadPromise).path(), 'utf8');
    await page.route('https://example.test/diagrams.html', (route) =>
      route.fulfill({ contentType: 'text/html', body: output }),
    );
    await page.route(/https:\/\/(?!example\.test)/, (route) => route.abort());
    await page.goto('https://example.test/diagrams.html');
    await expect(page.locator('article figure.su-diagram .su-diagram-view')).toHaveCount(3);
    const exportedModel = page.locator('figure[data-kind="stl"]');
    await exportedModel.getByRole('button', { name: 'Surface angle', exact: true }).click();
    await expect(exportedModel.getByRole('button', { name: 'Surface angle', exact: true })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    await expect(exportedModel.locator('svg polygon').first()).not.toHaveAttribute('fill', 'none');
    await expect(exportedModel.locator('svg polygon').first()).toHaveAttribute('stroke', 'none');
    await page.setViewportSize({ width: 390, height: 844 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
    await page.emulateMedia({ media: 'print' });
    await expect(exportedModel.locator('.su-diagram-view')).toBeVisible();
    await expect(exportedModel.locator('.su-diagram-controls')).toBeHidden();
  });

  test('opens binary and ASCII STL files as editable Markdown', async ({ page }) => {
    await page.goto(tool.path);
    const binary = Buffer.alloc(134);
    binary.write('solid binary header');
    binary.writeUInt32LE(1, 80);
    const values = [0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0];
    values.forEach((value, index) => binary.writeFloatLE(value, 84 + index * 4));
    await page.locator('#fileInput').setInputFiles({ name: 'triangle.stl', mimeType: 'model/stl', buffer: binary });
    const model = page.locator('#previewArticle figure[data-kind="stl"]');
    await expect(model.locator('polygon')).toHaveCount(1);
    await expect(page.locator('#docTitle')).toHaveText('triangle');
    await page.getByRole('button', { name: 'Source', exact: true }).click();
    await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
    const source = await page.evaluate(() =>
      (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText(),
    );
    expect(source).toContain('```stl\nsolid model');
    expect(source).toContain('vertex 1 0 0');
    await page.keyboard.press('Escape');
    await page.getByRole('button', { name: 'HTML', exact: true }).click();
    await expect(page.frameLocator('#htmlPreview').locator('figure[data-kind="stl"] polygon')).toHaveCount(1);
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    const ascii = source.slice(source.indexOf('\n') + 1, source.lastIndexOf('```')).trim();
    await page.locator('#fileInput').setInputFiles({
      name: 'ascii.STL',
      mimeType: 'model/stl',
      buffer: Buffer.from(ascii),
    });
    await expect(page.locator('#docTitle')).toHaveText('ascii');
    await expect(model.locator('polygon')).toHaveCount(1);
    await page.locator('#fileInput').setInputFiles({
      name: 'truncated.stl',
      mimeType: 'model/stl',
      buffer: binary.subarray(0, 120),
    });
    await expect(page.locator('#toast')).toContainText('Invalid or truncated STL file');
    await expect(page.locator('#docTitle')).toHaveText('ascii');
    await expect(model.locator('polygon')).toHaveCount(1);
  });

  test('large STL meshes use a bounded DOM and retain interactive HTML exports', async ({ page }) => {
    await page.goto(tool.path);
    const count = 12000;
    const binary = Buffer.alloc(84 + count * 50);
    binary.writeUInt32LE(count, 80);
    for (let i = 0; i < count; i++) {
      const x = (i % 100) / 100;
      const y = Math.floor(i / 100) / 120;
      [0, 0, 1, x, y, 0, x + 0.01, y, 0, x, y + 0.01, 0].forEach((value, index) =>
        binary.writeFloatLE(value, 84 + i * 50 + index * 4),
      );
    }
    await page.locator('#fileInput').setInputFiles({ name: 'large.stl', mimeType: 'model/stl', buffer: binary });
    const model = page.locator('figure[data-kind="stl"]');
    await expect(model.locator('canvas')).toBeVisible({ timeout: 15000 });
    await expect(model.locator('figcaption')).toHaveCount(0);
    expect(await model.locator('*').count()).toBeLessThan(100);
    await model.getByRole('button', { name: 'Wireframe', exact: true }).click();
    await model.getByRole('button', { name: 'Pan right', exact: true }).click();
    await model.getByRole('button', { name: 'Reset view' }).click();
    await model.getByRole('button', { name: 'Rotate right', exact: true }).click();
    const savedView = await model.getAttribute('data-current-view');
    await model.getByRole('button', { name: 'Save current view to source' }).click();
    await expect(model).toHaveAttribute('data-view', savedView!);
    await page.getByRole('button', { name: 'Source', exact: true }).click();
    await expect(page.locator('#editorMount :is(.cm-editor, .editor-fallback)')).toBeVisible();
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    await page.getByRole('button', { name: 'HTML', exact: true }).click();
    const exported = page.frameLocator('#htmlPreview');
    await expect(exported.locator('figure canvas')).toBeVisible({ timeout: 15000 });
    await expect(exported.locator('figure')).toHaveAttribute('data-view', savedView!);
    await expect(exported.locator('.su-stl-snapshot')).toHaveAttribute('src', /^data:image\/png/);
    await exported.getByRole('button', { name: 'Surface angle', exact: true }).click();
    await page.emulateMedia({ media: 'print' });
    await expect(exported.locator('.su-stl-snapshot')).toBeVisible();
  });

  test('3MF imports preserve face colors, source, share state, and offline HTML/print output', async ({ page }) => {
    await page.goto(tool.path);
    await page.locator('#fileInput').setInputFiles(modelFixture('face-colors'));
    const model = page.locator('#previewArticle figure[data-kind="su-mesh"]');
    await expect(model.locator('canvas')).toBeVisible({ timeout: 15000 });
    await expect(model.locator('figcaption')).toHaveCount(0);
    const pixels = await modelPixels(page);
    expect(pixels.red).toBeGreaterThan(100);
    expect(pixels.green).toBeGreaterThan(100);
    expect(pixels.blue).toBeGreaterThan(100);
    await model.getByRole('button', { name: 'Toggle viewer fullscreen' }).click();
    await expect.poll(() => page.evaluate(() => document.fullscreenElement?.getAttribute('data-kind'))).toBe('su-mesh');
    await model.getByRole('button', { name: 'Toggle viewer fullscreen' }).click();
    await model.getByRole('button', { name: 'Wireframe', exact: true }).click();
    await model.getByRole('button', { name: 'Surface angle', exact: true }).click();
    await model.getByRole('button', { name: 'Solid', exact: true }).click();
    expect(await modelPixels(page)).toEqual(pixels);
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
    await model.getByRole('button', { name: 'Copy source' }).click();
    const copied = await page.evaluate(() => navigator.clipboard.readText());
    expect(JSON.parse(copied)).toMatchObject({ version: 1, count: 44 });
    await page.getByRole('button', { name: 'Source', exact: true }).click();
    await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
    const source = await page.evaluate(() =>
      (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText(),
    );
    expect(source).toContain(`\`\`\`su-mesh\n${copied.trim()}`);
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    await page.getByRole('button', { name: 'Share', exact: true }).click();
    await expect(page.locator('#toast')).toContainText('Share link copied');
    const share = await page.evaluate(() => navigator.clipboard.readText());
    expect(share).toContain('/md#');
    await page.goto(share);
    await expect(model.locator('canvas')).toBeVisible({ timeout: 15000 });
    expect(await modelPixels(page)).toEqual(pixels);
    const downloadPromise = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Export', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Download HTML' }).click();
    const output = await readFile(await (await downloadPromise).path(), 'utf8');
    expect(output).not.toContain('data-su-bundle');
    await page.route('https://example.test/colored.html', (route) =>
      route.fulfill({ contentType: 'text/html', body: output }),
    );
    await page.route(/https:\/\/(?!example\.test)/, (route) => route.abort());
    await page.goto('https://example.test/colored.html');
    const exported = page.locator('figure[data-kind="su-mesh"]');
    await expect(exported.locator('canvas')).toBeVisible();
    await expect(exported.locator('figcaption')).toHaveCount(0);
    expect(await modelPixels(page)).toEqual(pixels);
    await exported.getByRole('button', { name: 'Pan right', exact: true }).click();
    await exported.getByRole('button', { name: 'Reset view' }).click();
    expect(await modelPixels(page)).toEqual(pixels);
    await page.setViewportSize({ width: 390, height: 844 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
    await page.emulateMedia({ media: 'print' });
    await expect(exported.locator('canvas')).toBeHidden();
    await expect(exported.locator('.su-stl-snapshot')).toBeVisible();
    await expect(exported.locator('.su-stl-snapshot')).toHaveAttribute('src', /^data:image\/png/);
    const printColors = await exported.locator('.su-stl-snapshot').evaluate(async (element) => {
      const image = element as HTMLImageElement;
      await image.decode();
      const canvas = document.createElement('canvas');
      canvas.width = image.naturalWidth;
      canvas.height = image.naturalHeight;
      const ctx = canvas.getContext('2d')!;
      ctx.drawImage(image, 0, 0);
      const data = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      let red = 0;
      for (let i = 0; i < data.length; i += 4) {
        if (data[i] > 80 && data[i] > data[i + 1] * 1.8 && data[i] > data[i + 2] * 1.8) red++;
      }
      return red;
    });
    expect(printColors).toBeGreaterThan(100);
    await expect(exported.locator('.su-diagram-controls')).toBeHidden();
  });

  test('3MF vertex colors interpolate, and uncolored/component/slicer files import', async ({ page }) => {
    await page.goto(tool.path);
    await page.locator('#fileInput').setInputFiles(modelFixture('vertex-colors'));
    const model = page.locator('figure[data-kind="su-mesh"]');
    await expect(model.locator('canvas')).toBeVisible({ timeout: 15000 });
    const pixels = await modelPixels(page);
    expect(pixels.red).toBeGreaterThan(100);
    expect(pixels.green).toBeGreaterThan(100);
    expect(pixels.blue).toBeGreaterThan(100);
    expect(pixels.mixed).toBeGreaterThan(100);
    for (const name of ['box', 'multiple-cylinders', 'prusaslicer']) {
      await page.locator('#fileInput').setInputFiles(modelFixture(name));
      await expect(page.locator('#docTitle')).toHaveText(name);
      await expect(model.locator('canvas')).toBeVisible();
      await expect(model.locator('figcaption')).toHaveCount(0);
    }
    await expect(page.locator('#toast')).toContainText('Slicer-specific paint metadata is unsupported');
    await page
      .locator('#fileInput')
      .setInputFiles({ name: 'broken.3mf', mimeType: 'model/3mf', buffer: Buffer.from('bad') });
    await expect(page.locator('#toast')).toContainText('Failed to open file');
    await expect(page.locator('#docTitle')).toHaveText('prusaslicer');
    await expect(model.locator('canvas')).toBeVisible();
  });

  test('ZIP64-packaged 3MF files preserve object defaults and triangle colors in HTML exports', async ({ page }) => {
    await page.goto(tool.path);
    await page.locator('#fileInput').setInputFiles(modelFixture('zip64-colors'));
    const model = page.locator('figure[data-kind="su-mesh"]');
    await expect(model.locator('canvas')).toBeVisible({ timeout: 15000 });
    await expect(model.locator('figcaption')).toHaveCount(0);
    const colors = await modelPixels(page);
    expect(colors.red).toBeGreaterThan(100);
    expect(colors.blue).toBeGreaterThan(100);
    await page.getByRole('button', { name: 'HTML', exact: true }).click();
    const exported = page.frameLocator('#htmlPreview').locator('figure[data-kind="su-mesh"]');
    await expect(exported.locator('canvas')).toBeVisible();
    const outputColors = await exported.locator('canvas').evaluate((element) => {
      const canvas = element as HTMLCanvasElement;
      const gl = canvas.getContext('webgl')!;
      const pixels = new Uint8Array(canvas.width * canvas.height * 4);
      gl.readPixels(0, 0, canvas.width, canvas.height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
      let red = 0;
      let blue = 0;
      for (let i = 0; i < pixels.length; i += 4) {
        if (pixels[i] > 80 && pixels[i] > pixels[i + 2] * 1.8) red++;
        if (pixels[i + 2] > 80 && pixels[i + 2] > pixels[i] * 1.8) blue++;
      }
      return { red, blue };
    });
    expect(outputColors.red).toBeGreaterThan(100);
    expect(outputColors.blue).toBeGreaterThan(100);
  });

  test('3MF import cancellation, replacement, and editing prevent stale results', async ({ page }) => {
    // Hold expensive work to make cancellation deterministic across machines.
    await page.addInitScript(() => {
      const OriginalWorker = window.Worker;
      window.Worker = class extends OriginalWorker {
        timer?: ReturnType<typeof setTimeout>;
        postMessage(message: unknown, transferOrOptions?: Transferable[] | StructuredSerializeOptions) {
          this.timer = setTimeout(() => {
            if (Array.isArray(transferOrOptions)) super.postMessage(message, transferOrOptions);
            else super.postMessage(message, transferOrOptions);
          }, 800);
        }
        terminate() {
          clearTimeout(this.timer);
          super.terminate();
        }
      };
    });
    await page.goto(tool.path);
    await setMarkdown(page, '# Keep this');
    await expect(page.locator('#previewArticle h1')).toContainText('Keep this');
    await page.locator('#fileInput').setInputFiles(modelFixture('face-colors'));
    await expect(page.getByRole('button', { name: 'Cancel import' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Export', exact: true })).toBeDisabled();
    await page.getByRole('button', { name: 'Cancel import' }).click();
    await expect(page.locator('#previewArticle h1')).toContainText('Keep this');
    await page.locator('#fileInput').setInputFiles(modelFixture('face-colors'));
    await expect(page.getByRole('button', { name: 'Cancel import' })).toBeVisible();
    await page
      .locator('#fileInput')
      .setInputFiles({ name: 'new.md', mimeType: 'text/markdown', buffer: Buffer.from('# New file') });
    await expect(page.locator('#previewArticle h1')).toContainText('New file');
    await page.locator('#fileInput').setInputFiles(modelFixture('face-colors'));
    await expect(page.getByRole('button', { name: 'Cancel import' })).toBeVisible();
    await setMarkdown(page, '# Edited during import');
    await expect(page.getByRole('button', { name: 'Cancel import' })).toBeHidden();
    // Wait beyond the held worker dispatch, then establish that no stale result won.
    await page.evaluate(() => new Promise((resolve) => setTimeout(resolve, 1000)));
    await expect(page.locator('#previewArticle h1')).toContainText('Edited during import');
    await expect(page.locator('figure[data-kind="su-mesh"]')).toHaveCount(0);
  });

  test('saved STL and 3MF views persist in source, HTML, and PDF', async ({ page }) => {
    await page.goto(tool.path);
    const stl =
      'solid triangle\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid triangle';
    await setMarkdown(page, `# Models\n\n\`\`\`stl\n${stl}\n\`\`\`\n\n\`\`\`stl\n${stl}\n\`\`\`\n`);
    const models = page.locator('#previewArticle figure[data-kind="stl"]');
    await expect(models).toHaveCount(2);
    const second = models.nth(1);
    await second.getByRole('button', { name: 'Rotate right', exact: true }).click();
    await second.getByRole('button', { name: 'Pan right', exact: true }).click();
    await second.getByRole('button', { name: 'Zoom in', exact: true }).click();
    await second.getByRole('button', { name: 'Wireframe', exact: true }).click();
    const rotated = await second.locator('polygon').getAttribute('points');
    await second.getByRole('button', { name: 'Save current view to source' }).click();
    await expect(page.locator('#toast')).toContainText('Current view saved to source');
    await expect(second.locator('polygon')).toHaveAttribute('points', rotated!);
    await expect(second.getByRole('button', { name: 'Wireframe', exact: true })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    const source = await page.evaluate(() =>
      (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText(),
    );
    expect(source).toContain(`\`\`\`stl\n${stl}`);
    expect(source).toContain(
      'stl view={"yaw":-0.39999999999999997,"pitch":0.5,"zoom":1.25,"panX":20,"panY":0,"mode":"Wireframe"}',
    );
    expect(source.match(/view=/g)).toHaveLength(1);
    await page.getByRole('button', { name: 'HTML', exact: true }).click();
    const htmlModels = page.frameLocator('#htmlPreview').locator('figure[data-kind="stl"]');
    await expect(htmlModels.nth(1).locator('polygon')).toHaveAttribute('points', rotated!);
    await expect(htmlModels.getByRole('button', { name: 'Save current view to source' })).toHaveCount(0);

    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    await page.locator('#fileInput').setInputFiles(modelFixture('face-colors'));
    const colored = page.locator('#previewArticle figure[data-kind="su-mesh"]');
    await expect(colored.locator('canvas')).toBeVisible();
    await colored.getByRole('button', { name: 'Rotate right', exact: true }).click();
    await colored.getByRole('button', { name: 'Pan down', exact: true }).click();
    await colored.getByRole('button', { name: 'Zoom in', exact: true }).click();
    await colored.getByRole('button', { name: 'Surface angle', exact: true }).click();
    const view = await colored.getAttribute('data-current-view');
    await colored.getByRole('button', { name: 'Save current view to source' }).click();
    await expect(colored).toHaveAttribute('data-view', view!);
    await page.getByRole('button', { name: 'HTML', exact: true }).click();
    const exported = page.frameLocator('#htmlPreview').locator('figure[data-kind="su-mesh"]');
    await expect(exported).toHaveAttribute('data-current-view', view!);
    const snapshot = await exported.locator('.su-stl-snapshot').getAttribute('src');
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    // Stub pagination/printing only; exercise the real PDF document generation.
    await page.route('**/paged.polyfill.js', (route) =>
      route.fulfill({
        contentType: 'text/javascript',
        body: 'window.__suPdfReady = true; window.print = () => {};',
      }),
    );
    await page.getByRole('button', { name: 'Export', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Download PDF' }).click();
    const pdf = page.frameLocator('iframe[aria-hidden="true"]');
    await expect(pdf.locator('figure .su-stl-snapshot')).toHaveAttribute('src', snapshot!);
    await expect(pdf.locator('figure')).toHaveAttribute('data-view', view!);
    await expect(pdf.locator('figure figcaption')).toHaveCount(0);
  });

  test('source view metadata accepts partial settings and rejects invalid model views', async ({ page }) => {
    await page.goto(tool.path);
    const stl =
      'solid triangle\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid triangle';
    await setMarkdown(page, `~~~stl view={"yaw":1,"panY":25}\n${stl}\n~~~`);
    const model = page.locator('#previewArticle figure[data-kind="stl"]');
    await expect(model).toHaveAttribute('data-view', /"yaw":1/);
    await model.getByRole('button', { name: 'Save current view to source' }).click();
    await expect(page.locator('#toast')).toContainText('Current view saved to source');
    const source = await page.evaluate(() =>
      (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText(),
    );
    expect(source).toMatch(/^~~~stl view=/);
    for (const view of ['{"zoom":0}', '{"yaw":"bad"}', '{"mode":"unknown"}', '{"other":1}', 'broken']) {
      await setMarkdown(page, `\`\`\`stl view=${view}\n${stl}\n\`\`\`\n\n## Later`);
      await expect(page.locator('#previewArticle .su-mermaid-error')).toBeVisible();
      await expect(page.locator('#previewArticle h2')).toContainText('Later');
    }
  });

  test('invalid map and STL fences preserve source and later content', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(
      page,
      '```geojson\n{"type":"Point","coordinates":["bad",0]}\n```\n\n' +
        '```topojson\n{"type":"Topology","objects":{}}\n```\n\n' +
        '```stl\nsolid empty\nendsolid\n```\n\n## Still readable\n\nBody.',
    );
    await expect(page.locator('#previewArticle .su-mermaid-error')).toHaveCount(3);
    await expect(page.locator('#previewArticle pre')).toHaveCount(3);
    await expect(page.locator('#previewArticle h2')).toContainText('Still readable');
  });

  test('preview block edits splice the source, undo, and survive a reload', async ({ page }) => {
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
    await page.goto(tool.path);
    await setMarkdown(page, md);
    const source = () => page.evaluate(() => (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText());
    const article = page.locator('#previewArticle');
    const blockAction = async (block: ReturnType<typeof article.locator>, name: string) => {
      await block.hover();
      await page.locator('#blockGripBtn').click();
      await page.getByRole('menuitem', { name, exact: true }).click();
    };

    // Toggling a checkbox changes exactly one character.
    await article.locator('li', { hasText: 'Task A' }).locator('input[type="checkbox"]').click();
    let expected = md.replace('- [ ] Task A', '- [x] Task A');
    await expect.poll(source).toBe(expected);
    await expect(page.locator('#docStats')).toContainText('4 of 5 left');

    // Done on a heading strikes it through and collapses its section.
    await blockAction(article.locator('h2', { hasText: '2. Second' }), 'Mark done');
    expected = expected.replace('## 2. Second', '## ~~2. Second~~');
    await expect.poll(source).toBe(expected);
    await expect(article.getByText('Second body.')).toBeHidden();
    await expect(page.locator('#docStats')).toContainText('3 of 5 left');

    // Deleting a section leaves every other byte untouched, and undo restores it.
    await blockAction(article.locator('h2', { hasText: '3. Third' }), 'Delete section');
    await expect.poll(source).toBe(expected.replace('\n## 3. Third\n\nThird body.\n', ''));
    await page.locator('#docStats').click();
    await page.keyboard.press('ControlOrMeta+z');
    await expect.poll(source).toBe(expected);

    // Double-click edits a block as Markdown; Escape commits and re-renders it.
    await article.getByText('Keep this paragraph.').dblclick();
    await expect(page.locator('.su-block-editor')).toBeVisible();
    await page.keyboard.press('ControlOrMeta+a');
    await page.keyboard.type('Edited **paragraph**.');
    await page.keyboard.press('Escape');
    expected = expected.replace('Keep this paragraph.', 'Edited **paragraph**.');
    await expect.poll(source).toBe(expected);
    await expect(article.locator('strong')).toHaveText('paragraph');

    // "+" on a task item adds a sibling task with a prefilled marker.
    await article.locator('li', { hasText: 'Task B' }).hover();
    await page.locator('#blockAddBtn').click();
    await page.keyboard.type('Task C');
    await page.keyboard.press('Escape');
    expected = expected.replace('- [ ] Task B', '- [ ] Task B\n- [ ] Task C');
    await expect.poll(source).toBe(expected);

    // Edits are autosaved to the URL fragment, so a reload keeps them.
    await expect.poll(() => page.evaluate('decodeState(location.hash.slice(1))')).toBe(expected);
    await page.reload();
    await expect(article.locator('li', { hasText: 'Task C' })).toBeVisible();
    await expect.poll(source).toBe(expected);
  });

  test('block handle stays on its list item, drags to reorder, and selections format', async ({ page }) => {
    const md = '# Doc\n\nPlain words here.\n\n- [ ] Task A\n- [ ] Task B\n- [ ] Task C\n';
    await page.goto(tool.path);
    await setMarkdown(page, md);
    const source = () => page.evaluate(() => (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText());
    const article = page.locator('#previewArticle');
    const grip = page.locator('#blockGripBtn');

    // Reaching for the handle across the list's left padding keeps it on Task C.
    const taskC = article.locator('li', { hasText: 'Task C' });
    await taskC.hover();
    const itemBox = (await taskC.boundingBox())!;
    const gripBox = (await grip.boundingBox())!;
    await page.mouse.move(gripBox.x + gripBox.width / 2, gripBox.y + gripBox.height / 2, { steps: 10 });
    expect(Math.abs((await grip.boundingBox())!.y - gripBox.y)).toBeLessThan(1);
    expect(gripBox.y).toBeGreaterThanOrEqual(itemBox.y - 4);

    // Dragging the grip above Task A moves Task C to the top of the list.
    const taskA = (await article.locator('li', { hasText: 'Task A' }).boundingBox())!;
    await page.mouse.down();
    await page.mouse.move(gripBox.x + 40, taskA.y + 2, { steps: 8 });
    await page.mouse.up();
    let expected = md.replace('- [ ] Task A\n- [ ] Task B\n- [ ] Task C\n', '- [ ] Task C\n- [ ] Task A\n- [ ] Task B\n');
    await expect.poll(source).toBe(expected);
    await expect(article.locator('li').first()).toContainText('Task C');

    // Selecting text shows the toolbar; a format wraps the selection and toggles back off.
    const formatText = async (needle: string, name: string, result: string) => {
      await page.evaluate((text) => {
        const p = document.querySelector('#previewArticle p')!;
        const from = p.textContent!.indexOf(text);
        const to = from + text.length;
        const walker = document.createTreeWalker(p, NodeFilter.SHOW_TEXT);
        const range = document.createRange();
        let pos = 0;
        let started = false;
        for (let node = walker.nextNode(); node; node = walker.nextNode()) {
          const len = (node as Text).length;
          if (!started && from <= pos + len) { range.setStart(node, from - pos); started = true; }
          if (to <= pos + len) { range.setEnd(node, to - pos); break; }
          pos += len;
        }
        getSelection()!.removeAllRanges();
        getSelection()!.addRange(range);
      }, needle);
      await page.locator('#formatBar').getByRole('button', { name }).click();
      expected = expected.replace(/^.*here\..*$/m, result);
      await expect.poll(source).toBe(expected);
      await expect.poll(() => page.evaluate('renderedMarkdown === markdown')).toBe(true);
    };
    await formatText('words', 'Bold', 'Plain **words** here.');
    await formatText('words', 'Bold', 'Plain words here.');
    // A selection across an already struck run strikes it as one run, then clears it.
    await formatText('words', 'Strikethrough', 'Plain ~~words~~ here.');
    await formatText('Plain words here.', 'Strikethrough', '~~Plain words here.~~');
    await formatText('Plain words here.', 'Strikethrough', 'Plain words here.');
    await formatText('Plain words here.', 'Highlight', '==Plain words here.==');
    await formatText('words', 'Highlight', '==Plain== words ==here.==');
  });

  test('block edits keep CRLF line endings', async ({ page }) => {
    const md = '---\r\ntitle: Win\r\n---\r\n# Win\r\n\r\n- [ ] Task A\r\n- [ ] Task B\r\n';
    await page.goto(tool.path);
    const encoded = await page.evaluate(`encodeState(${JSON.stringify(md)})`);
    await page.goto(`${tool.path}#${encoded}`);
    await page.reload();

    await page.locator('#previewArticle li', { hasText: 'Task A' }).locator('input[type="checkbox"]').click();
    await expect
      .poll(() => page.evaluate('decodeState(location.hash.slice(1))'))
      .toBe(md.replace('- [ ] Task A', '- [x] Task A'));
  });

  test('comments anchor to selections, thread replies, and persist with the file name', async ({ page }) => {
    const md = '# Review\n\nI disagree with warnings as the final behavior.\n\nSecond paragraph.\n';
    await page.goto(tool.path);
    const encoded = await page.evaluate(`encodeState(${JSON.stringify(md)})`);
    await page.goto(`${tool.path}#name=review.replies.md&md=${encoded}`);
    await page.reload();
    await expect(page.locator('#docTitle')).toHaveText('review.replies');
    const state = async () =>
      (await page.evaluate(
        `(async () => { const p = new URLSearchParams(location.hash.slice(1)); return { name: p.get('name'), md: await decodeState(p.get('md') ?? '') }; })()`,
      )) as { name: string | null; md: string | null };
    const highlighted = () =>
      page.evaluate(() =>
        ['su-comment', 'su-comment-active'].reduce((n, name) => n + (CSS.highlights.get(name)?.size ?? 0), 0),
      );

    // A selection opens a draft beside it; Markdown in the comment renders.
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
    await expect.poll(async () => (await state()).md).toContain('Keep it.');
    const saved = (await state()).md!;
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

    // A reload restores the name and threads; clicking the highlight opens the thread.
    await page.reload();
    await expect(page.locator('#docTitle')).toHaveText('review.replies');
    await expect(card.locator('.cm-comment')).toHaveCount(2);
    await expect.poll(highlighted).toBe(1);
    const point = await page.evaluate(
      '(() => { const r = [...anchors.values()][0].getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()',
    );
    const { x, y } = point as { x: number; y: number };
    await page.mouse.click(x, y);
    await expect(card).toHaveClass(/active/);

    // The title toggles a thread; collapsed mode folds every inactive thread to one line.
    const title = card.locator('.cm-quote');
    await title.click();
    await expect(title).toHaveAttribute('aria-expanded', 'false');
    await title.click();
    await expect(title).toHaveAttribute('aria-expanded', 'true');
    const modeButton = page.locator('#commentsBtn');
    await modeButton.click();
    await expect(modeButton).toHaveAttribute('data-mode', 'collapsed');
    await expect(title).toHaveAttribute('aria-expanded', 'true');
    await page.keyboard.press('Escape');
    await expect(title).toHaveAttribute('aria-expanded', 'false');
    await expect(card.locator('.cm-count')).toHaveText('2');
    await modeButton.click();
    await expect(page.locator('#commentsRail')).toBeHidden();
    await expect.poll(highlighted).toBe(0);
    await modeButton.click();
    await expect(modeButton).toHaveAttribute('data-mode', 'expanded');
    await expect(title).toHaveAttribute('aria-expanded', 'true');

    await card.getByRole('button', { name: 'Resolve thread' }).click();
    await expect(card).toHaveClass(/resolved/);
    await expect.poll(highlighted).toBe(0);

    // Renaming the title updates the name recorded in the URL.
    await page.locator('#docTitle').click();
    await page.keyboard.press('ControlOrMeta+a');
    await page.keyboard.type('Final notes');
    await page.keyboard.press('Enter');
    await expect.poll(async () => (await state()).name).toBe('Final notes');
  });

  test('find-in-page highlights matches', async ({ page }) => {
    await page.goto(tool.path);
    await setMarkdown(page, 'alpha beta alpha gamma alpha');

    await page.getByRole('button', { name: 'Find in document' }).click();
    await page.getByPlaceholder('Find in document').fill('alpha');

    await expect(page.locator('#previewArticle mark.find-hit')).toHaveCount(3);
    await expect(page.locator('#findCount')).toContainText('/3');
  });

  test('loads Markdown from the URL fragment', async ({ page }) => {
    // base64url of "| len(LE u32) | deflate-raw('# Frag load') |" for "# Frag load".
    const md = '# Frag load\n\nFrom the fragment.';
    const encoded = await page.evaluate(async (text: string) => {
      const bytes = new TextEncoder().encode(text);
      const stream = new Blob([bytes]).stream().pipeThrough(new CompressionStream('deflate-raw'));
      const deflated = new Uint8Array(await new Response(stream).arrayBuffer());
      const out = new Uint8Array(4 + deflated.length);
      new DataView(out.buffer).setUint32(0, bytes.length, true);
      out.set(deflated, 4);
      let s = '';
      for (const b of out) s += String.fromCharCode(b);
      return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
    }, md);

    await page.goto(`${tool.path}#${encoded}`);
    await expect(page.locator('#previewArticle h1')).toContainText('Frag load');
    // The empty "start" panel and the loading state must not linger once hydrated.
    await expect(page.locator('#emptyState')).toBeHidden();
    await expect(page.locator('#loadingState')).toBeHidden();
  });

  test('Load URL extracts embedded source from an exported HTML document', async ({ page }) => {
    // An exported HTML doc carrying the "Embed source" <script type="text/markdown">.
    // The </script> inside the source is escaped to <\/script>, exactly as the
    // tool's exporter writes it; the loader must unescape it on the way back.
    const html = [
      '<!DOCTYPE html><html><head><title>x</title></head><body>',
      '<main><article><h1>Rendered</h1></article></main>',
      '<script type="text/markdown" id="su-md-source" data-source="markdown">',
      '# Round trip\n\nConsole: `console.log("<\\/script>")`',
      '</script>',
      '</body></html>',
    ].join('\n');

    await page.route('https://example.com/exported.html', (route) =>
      route.fulfill({ contentType: 'text/html', body: html }),
    );

    await page.goto(tool.path);
    await page.getByLabel('Markdown URL to load').fill('https://example.com/exported.html');
    await page.getByRole('button', { name: 'Load URL' }).click();

    // The embedded Markdown is rendered, not the document's own <h1>Rendered</h1>.
    await expect(page.locator('#previewArticle h1')).toContainText('Round trip');
    await expect(page.locator('#previewArticle code')).toContainText('console.log("</script>")');
  });
});
