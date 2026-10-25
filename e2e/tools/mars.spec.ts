import { deflateRawSync } from 'node:zlib';

import type { Browser, BrowserContext, Page } from '@playwright/test';
import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';
import { selectionToolbarTests } from './_markdown_selection';

// MARS keeps reviews in IndexedDB, so every test gets a fresh context. It runs on its own
// host (MARS_BASE_URL); locally, `npm run serve` in dev/tools/js/mars and
// MARS_BASE_URL=http://localhost:7373.
const tool = getTool('mars');
const MARS_BASE_URL = process.env.MARS_BASE_URL ?? `https://${tool.host}`;
test.use({ baseURL: MARS_BASE_URL });

async function setMarkdown(page: Page, md: string): Promise<void> {
  await page.waitForFunction(() => typeof (window as { __suEditorAPI?: unknown }).__suEditorAPI !== 'undefined');
  await page.evaluate((text) => {
    (window as unknown as { __suEditorAPI: { setText(t: string): void } }).__suEditorAPI.setText(text);
  }, md);
}

async function newDoc(page: Page, md: string) {
  await page.getByRole('button', { name: 'New document, folder, or import' }).click();
  await page.getByRole('menuitem', { name: 'New document' }).click();
  await expect(page.getByRole('button', { name: 'Source' })).toHaveAttribute('aria-pressed', 'true');
  await setMarkdown(page, md);
  await page.keyboard.press('Escape');
  await expect(page.locator('#previewArticle')).toContainText(md.replace(/^#\s*/, '').split('\n')[0]);
  await expect(page.locator('#saveState')).toHaveText('Saved');
}

const row = (page: Page, name: string) =>
  page.locator('#tree .tree-row').filter({ has: page.locator('.tree-label').getByText(name, { exact: true }) });

async function select(page: Page, needle: string) {
  await page.evaluate((text) => {
    const p = Array.from(document.querySelectorAll('#previewArticle p')).find((el) => el.textContent!.includes(text))!;
    const node = p.firstChild as Text;
    const from = node.data.indexOf(text);
    const range = document.createRange();
    range.setStart(node, from);
    range.setEnd(node, from + text.length);
    getSelection()!.removeAllRanges();
    getSelection()!.addRange(range);
  }, needle);
}

async function comment(page: Page, needle: string, body: string, author = 'Reviewer') {
  await select(page, needle);
  await page.locator('#formatBar').getByRole('button', { name: 'Comment' }).click();
  const draft = page.locator('.cm-card.draft');
  await draft.getByLabel('Your name').fill(author);
  await draft.getByRole('textbox', { name: 'Comment' }).fill(body);
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(page.locator('.cm-card .cm-body', { hasText: body })).toBeVisible();
}

async function copyLink(page: Page, item: RegExp) {
  await page.getByRole('button', { name: 'Share', exact: true }).click();
  await page.getByRole('menuitem', { name: item }).click();
  await expect(page.locator('#toast')).toContainText(/copied/i);
  return page.evaluate(() => navigator.clipboard.readText());
}

// The link the skill's `mars` shell function builds for a local file.
function fileLink(md: string, origin: string, name = 'Plan') {
  const utf8 = Buffer.from(md);
  const state = Buffer.concat([Buffer.alloc(4), deflateRawSync(utf8)]);
  state.writeUInt32LE(utf8.length, 0);
  return `/#${new URLSearchParams({ name, md: state.toString('base64url'), o: origin })}`;
}

const docRows = (page: Page) => page.locator('#tree .tree-row[data-kind="doc"]');

async function editSource(page: Page, md: string) {
  await page.getByRole('button', { name: 'Source', exact: true }).click();
  await setMarkdown(page, md);
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
  await expect(page.locator('#saveState')).toHaveText('Saved');
}

const PLAN = ['# Plan', '', 'First point stays.', '', 'Second point stays.', '', 'Third point stays.', ''];
const planWith = (last: string) => [...PLAN, last, ''].join('\n');

const threadsBlock = (threads: object[]) => `\n<!-- su-comments\n${JSON.stringify(threads, null, 2)}\n-->\n`;

function freshContext(browser: Browser): Promise<BrowserContext> {
  return browser.newContext({
    baseURL: MARS_BASE_URL,
    permissions: ['clipboard-read', 'clipboard-write'],
  });
}

test.describe(`${tool.name} (${tool.path})`, () => {
  selectionToolbarTests(async (page, markdown) => {
    await page.goto(tool.path);
    await page.getByRole('button', { name: 'Start writing' }).click();
    await setMarkdown(page, markdown);
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    await expect(page.locator('#previewArticle p').first()).toHaveText('Plain words here.');
  });

  test('SEO head block matches the AGENTS.md SEO budget', async ({ page }) => {
    const response = await page.goto(tool.path);
    expect(response?.ok()).toBeTruthy();
    await assertSeoBasics(page, tool);
  });

  test('skill .md is reachable with required frontmatter', async ({ request }) => {
    await assertSkillMd(request, tool);
  });

  test('documents persist in the sidebar across reloads', async ({ page }) => {
    await page.goto(tool.path);
    await expect(page.locator('#emptyState')).toBeVisible();
    await expect(page.locator('#sbUsage')).toHaveText('0 documents');
    await page.getByRole('button', { name: 'Load example' }).click();
    await expect(page.locator('#previewArticle h1')).toHaveText(/^Welcome to MARS/);
    await expect(row(page, 'Welcome to MARS')).toHaveAttribute('aria-selected', 'true');
    await expect(page).toHaveURL(/#doc=[\w-]+$/);
    await page.reload();
    await expect(page.locator('#previewArticle h1')).toHaveText(/^Welcome to MARS/);
    await page.goto(tool.path);
    await expect(page.locator('#previewArticle h1')).toHaveText(/^Welcome to MARS/);
  });

  test('folders: create, rename, move by drag, and delete with undo', async ({ page }) => {
    await page.goto(tool.path);
    await newDoc(page, '# Plan\n\nBody.');
    await expect(row(page, 'Untitled')).toBeVisible();
    await row(page, 'Untitled').press('F2');
    await page.locator('.tree-rename').fill('Plan');
    await page.keyboard.press('Enter');
    await expect(page.locator('#docTitle')).toHaveText('Plan');

    await page.getByRole('button', { name: 'New document, folder, or import' }).click();
    await page.getByRole('menuitem', { name: 'New folder' }).click();
    await page.locator('.tree-rename').fill('RFCs');
    await page.keyboard.press('Enter');

    await row(page, 'Plan').dragTo(row(page, 'RFCs'));
    await expect(row(page, 'Plan')).toHaveAttribute('aria-level', '2');
    await expect(page.locator('#crumbs')).toHaveText('RFCs');

    await row(page, 'RFCs')
      .getByRole('button', { name: /Actions for RFCs/ })
      .click();
    await page.getByRole('menuitem', { name: 'Delete' }).click();
    await page.getByRole('button', { name: 'Delete folder' }).click();
    await expect(row(page, 'Plan')).toHaveCount(0);
    await expect(page.locator('#emptyState')).toBeVisible();
    await page.locator('#toast').getByRole('button', { name: 'Undo' }).click();
    await expect(row(page, 'Plan')).toBeVisible();
    await expect(page.locator('#previewArticle h1')).toHaveText(/^Plan/);
  });

  test('a shared full link lands in "Shared with me" and comments merge back', async ({ page, context, browser }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.goto(tool.path);
    await newDoc(page, '# Review\n\nI disagree with warnings as the final behavior.');
    await comment(page, 'warnings as the final behavior', 'Too strong?', 'Author');
    await expect(page.locator('#tree .tree-badge')).toHaveText('1');
    const link = await copyLink(page, /^Copy full link/);
    expect(link).toMatch(/\/#name=.+&md=.+&o=.+/);

    const other = await freshContext(browser);
    const colleague = await other.newPage();
    await colleague.goto(link);
    await expect(colleague.locator('#previewArticle h1')).toHaveText(/^Review/);
    await expect(row(colleague, 'Shared with me')).toBeVisible();
    await expect(colleague.locator('#crumbs')).toHaveText('Shared with me');
    const card = colleague.locator('.cm-card');
    await card.getByRole('textbox', { name: 'Reply' }).fill('Agreed, softening it.');
    await card.getByRole('button', { name: 'Reply' }).click();
    await expect(card.locator('.cm-comment')).toHaveCount(2);
    const back = await copyLink(colleague, /^Copy full link/);
    await other.close();

    await page.goto(back);
    await expect(page.locator('#toast')).toContainText('Merged 1 new comment');
    await expect(page.locator('.cm-card .cm-comment')).toHaveCount(2);
    await expect(row(page, 'Shared with me')).toHaveCount(0);
  });

  test('a folder short link carries every document', async ({ page, context, browser }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.goto(tool.path);
    await newDoc(page, '# One');
    await newDoc(page, '# Two');
    await page.getByRole('button', { name: 'New document, folder, or import' }).click();
    await page.getByRole('menuitem', { name: 'New folder' }).click();
    await page.locator('.tree-rename').fill(`Batch ${Date.now()}`);
    await page.keyboard.press('Enter');
    const folder = page.locator('#tree .tree-row[data-kind="folder"]');
    for (const name of ['Untitled', 'Untitled (2)']) await row(page, name).dragTo(folder);
    await expect(page.locator('#tree .tree-row[aria-level="2"]')).toHaveCount(2);
    await folder.getByRole('button', { name: /^Actions for/ }).click();
    await page.getByRole('menuitem', { name: /^Copy short link/ }).click();
    await expect(page.locator('#toast')).toContainText('Short link copied');
    const link = await page.evaluate(() => navigator.clipboard.readText());
    expect(link).toMatch(/\/#s=[A-Za-z0-9_-]{16}$/);

    const other = await freshContext(browser);
    const colleague = await other.newPage();
    await colleague.goto(link);
    await expect(colleague.locator('#toast')).toContainText(/added 2 documents/);
    await expect(colleague.locator('#tree .tree-row[data-kind="doc"]')).toHaveCount(2);
    await expect(colleague.locator('#tree .tree-row[data-kind="folder"]')).toHaveCount(2);
    await other.close();
  });

  test('md.html links open, and Export all downloads a zip that imports back', async ({ page, browser }) => {
    await page.goto(tool.path);
    const encoded = await page.evaluate(async () => {
      const bytes = new TextEncoder().encode('# From md\n\nHello.');
      const deflated = new Uint8Array(
        await new Response(new Blob([bytes]).stream().pipeThrough(new CompressionStream('deflate-raw'))).arrayBuffer(),
      );
      const out = new Uint8Array(4 + deflated.length);
      new DataView(out.buffer).setUint32(0, bytes.length, true);
      out.set(deflated, 4);
      return btoa(Array.from(out, (b) => String.fromCharCode(b)).join(''))
        .replace(/\+/g, '-')
        .replace(/\//g, '_')
        .replace(/=+$/, '');
    });
    await page.goto(`/#name=Plan.md&md=${encoded}`);
    await expect(page.locator('#previewArticle h1')).toHaveText(/^From md/);
    await expect(page.locator('#docTitle')).toHaveText('Plan');
    const download = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Export all' }).click();
    const file = await download;
    expect(file.suggestedFilename()).toMatch(/^MARS documents \d{4}-\d{2}-\d{2}\.zip$/);
    const bytes = await (await import('node:fs/promises')).readFile((await file.path())!);
    expect(bytes.subarray(0, 2).toString()).toBe('PK');
    expect(bytes.includes(Buffer.from('Shared with me/Plan.md'))).toBe(true);

    // The export imports back with its folders.
    const other = await freshContext(browser);
    const restored = await other.newPage();
    await restored.goto(tool.path);
    await restored
      .locator('#importInput')
      .setInputFiles({ name: file.suggestedFilename(), mimeType: 'application/zip', buffer: bytes });
    await expect(restored.locator('#toast')).toContainText('Imported 1 document');
    await expect(row(restored, 'Shared with me')).toBeVisible();
    await expect(row(restored, 'Plan')).toHaveAttribute('aria-level', '2');
    await other.close();
  });

  test('stacked formats toggle from one selection, and the toolbar works by touch', async ({ page }) => {
    await page.goto(tool.path);
    await newDoc(page, 'Plain words here.');
    const source = () =>
      page.evaluate(() => (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText());
    await select(page, 'words');
    const toggle = async (name: string, result: string) => {
      await page.locator('#formatBar').getByRole('button', { name }).click();
      await expect.poll(source).toBe(result);
      await expect(page.locator('#formatBar')).toBeVisible();
      await expect.poll(() => page.evaluate(() => getSelection()!.toString())).toBe('words');
    };
    await toggle('Highlight', 'Plain ==words== here.');
    await toggle('Strikethrough', 'Plain ~~==words==~~ here.');
    await toggle('Highlight', 'Plain ~~words~~ here.');
    await toggle('Strikethrough', 'Plain words here.');

    // A tap on the toolbar formats the selection instead of collapsing it first.
    const touch = await page
      .context()
      .browser()!
      .newContext({
        baseURL: MARS_BASE_URL,
        hasTouch: true,
        isMobile: true,
        viewport: { width: 412, height: 800 },
      });
    const phone = await touch.newPage();
    await phone.goto(tool.path);
    await phone.getByRole('button', { name: 'Start writing' }).click();
    await setMarkdown(phone, 'Plain words here.');
    await phone.getByRole('button', { name: 'Preview', exact: true }).tap();
    await expect(phone.locator('#previewArticle p')).toHaveText('Plain words here.');
    await select(phone, 'words');
    await expect(phone.locator('#formatBar')).toBeVisible();
    await phone.locator('#formatBar').getByRole('button', { name: 'Bold' }).tap();
    await expect
      .poll(() =>
        phone.evaluate(() => (window as unknown as { __suEditorAPI: { getText(): string } }).__suEditorAPI.getText()),
      )
      .toBe('Plain **words** here.');
    await touch.close();
  });

  test('GitHub copy is a themed badge that links to the review', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.goto(tool.path);
    await newDoc(page, '# Notes\n\nThe cache needs a TTL.');
    const text = await copyLink(page, /^Copy for GitHub/);
    expect(text).toMatch(
      /^<a href="[^"]+\/#s=[\w-]+"><picture><source media="\(prefers-color-scheme: dark\)" srcset="[^"]+\/badge-dark\.svg"><img alt="View full review in MARS" src="[^"]+\/badge\.svg" height="28"><\/picture><\/a>$/,
    );
    for (const name of ['badge.svg', 'badge-dark.svg']) {
      const res = await page.request.get(name);
      expect(res.headers()['content-type']).toContain('image/svg+xml');
      expect(await res.text()).toContain('View full review in');
    }
  });

  test('GitHub threads copy lists open threads under a review link', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.goto(tool.path);
    await newDoc(page, '# Notes\n\nThe cache needs a TTL.');
    await comment(page, 'needs a TTL', 'Which TTL?');
    const text = await copyLink(page, /^Copy threads for GitHub/);
    expect(text).toMatch(/^\*\*\[Untitled\]\(.+\/#s=.+\)\*\* · review in MARS · 1 open thread/);
    expect(text).toContain('> needs a TTL');
    expect(text).toContain('Which TTL?');
  });

  test('a revised copy shows what changed, threads follow the rewrite, and Mark reviewed clears it', async ({
    page,
  }) => {
    const origin = `file:/tmp/plan-${Date.now()}.md`;
    const thread = {
      id: '1f3a9c2e',
      quote: 'warnings as the final behavior',
      comments: [{ id: 'a1b2c3d4', author: 'Reviewer', at: '2026-10-21T09:41:00.000Z', body: 'Too strong?' }],
    };
    const reply = { id: '5e6f7a8b', author: 'Agent', at: '2026-10-21T10:02:00.000Z', body: 'Queries fail closed now.' };
    await page.goto(tool.path);
    await page.evaluate(() => localStorage.setItem('su-md-author', 'Reviewer'));
    await page.goto(
      fileLink(
        `# Plan\n\nKeep this intro.\n\nI disagree with warnings as the final behavior.\n\nThis paragraph goes away.\n${threadsBlock([thread])}`,
        origin,
      ),
    );
    await expect(page.locator('#previewArticle h1')).toHaveText(/^Plan/);
    await expect(page.locator('.cm-card:not(.orphan)')).toHaveCount(1);
    await expect(page.locator('#changesBar')).toBeHidden();

    await page.goto(
      fileLink(
        `# Plan\n\nA new lead paragraph.\n\nKeep this intro.\n\nProtected queries fail closed instead of returning partial data.\n${threadsBlock([{ ...thread, comments: [...thread.comments, reply] }])}`,
        origin,
      ),
    );
    // Only the source changed since it was opened, so MARS takes it without asking.
    await expect(page.locator('#toast')).toContainText('showing what changed');
    await expect(page.locator('#changesLabel')).toHaveText('3 changes since your review · 1 new reply');
    await expect(page.locator('#previewArticle .su-diff-add')).toHaveCount(1);
    await expect(page.locator('#previewArticle .su-diff-edit')).toHaveCount(1);
    await expect(page.locator('#previewArticle .su-diff-del-end')).toHaveCount(1);
    await expect(page.locator('#tree .tree-dot')).toHaveCount(1);
    const card = page.locator('.cm-card');
    await expect(card).not.toHaveClass(/orphan/);
    await expect(card.locator('.cm-note')).toHaveText('Quoted text changed since your review');
    await expect(card.locator('.cm-new')).toHaveCount(1);

    await page.getByRole('button', { name: 'Next change' }).click();
    await expect(page.locator('#changesLabel')).toHaveText('Added · 1 of 3 · 1 new reply');
    await page.getByRole('button', { name: 'Next change' }).click();
    await expect(page.locator('#changesLabel')).toHaveText('Edited · 2 of 3 · 1 new reply');
    await page.getByRole('button', { name: 'Show before' }).click();
    await expect(page.locator('#diffBefore')).toContainText('I disagree with warnings as the final behavior.');

    await page.getByRole('button', { name: 'Mark reviewed' }).click();
    await expect(page.locator('#changesBar')).toBeHidden();
    await expect(page.locator('#tree .tree-dot')).toHaveCount(0);
    // The thread re-quotes to the inserted words of the edited paragraph.
    await expect(card.locator('.cm-quote')).toHaveText(
      'Protected queries fail closed instead of returning partial data',
    );
    await expect(page.locator('#saveState')).toHaveText('Saved');
    await page.reload();
    await expect(page.locator('.cm-card')).not.toHaveClass(/orphan/);
    await expect(page.locator('#changesBar')).toBeHidden();
    await expect(page.locator('.cm-new')).toHaveCount(0);
  });

  test('Copy for agent hands over the file, the open threads, and the reply steps', async ({ page, context }) => {
    await context.grantPermissions(['clipboard-read', 'clipboard-write']);
    const open = {
      id: '1f3a9c2e',
      quote: 'needs a TTL',
      comments: [{ id: 'a1b2c3d4', author: 'Oleg', at: '2026-10-21T09:41:00.000Z', body: 'Which TTL?' }],
    };
    const done = { ...open, id: '2f3a9c2e', quote: 'The cache', resolved: true };
    await page.goto(fileLink(`The cache needs a TTL.\n${threadsBlock([open, done])}`, 'file:/tmp/agent.md'));
    await expect(page.locator('.cm-card')).toHaveCount(2);
    const text = await copyLink(page, /^Copy for agent/);
    expect(text).toMatch(/^Address the 1 open review thread on \/tmp\/agent\.md\. .+\/llms\.txt$/m);
    expect(text).toContain('append one comment');
    expect(text).toContain('<!-- su-comments');
    expect(text).toContain('"id": "1f3a9c2e"');
    expect(text).not.toContain('2f3a9c2e');
  });

  test('Open and moved files update the same document after the reader confirms', async ({ page }) => {
    await page.goto(fileLink(planWith('Fourth point, first draft.'), 'file:/tmp/dl/test.md', 'test'));
    await expect(page.locator('#toast')).toContainText('Added "test"');
    await expect(page.locator('#previewArticle')).toContainText('first draft');
    // The import's last save can re-render the tree between focusing the row and the key press.
    await expect(async () => {
      if (!(await page.locator('.tree-rename').count())) await row(page, 'test').press('F2');
      await expect(page.locator('.tree-rename')).toBeVisible({ timeout: 1000 });
    }).toPass();
    await page.locator('.tree-rename').fill('test v1');
    await page.keyboard.press('Enter');

    // Open without a file handle (Firefox, or the file input): matched by content.
    await page.locator('#fileInput').setInputFiles({
      name: 'test.md',
      mimeType: 'text/markdown',
      buffer: Buffer.from(planWith('Fourth point, second draft.')),
    });
    await expect(page.getByRole('heading', { name: 'Is this an updated copy?' })).toBeVisible();
    await page.getByRole('button', { name: 'Update "test v1"' }).click();
    await expect(page.locator('#toast')).toContainText('showing what changed');
    await expect(page.locator('#changesLabel')).toHaveText('1 change since your review');
    await expect(docRows(page)).toHaveCount(1);

    // The file moved: confirm once, then its new path matches directly.
    const moved = 'file:/tmp/dl/tests/test.md';
    await page.goto(fileLink(planWith('Fourth point, third draft.'), moved, 'test'));
    await page.getByRole('button', { name: 'Update "test v1"' }).click();
    await expect(page.locator('#previewArticle')).toContainText('third draft');
    await page.goto(fileLink(planWith('Fourth point, final.'), moved, 'test'));
    await expect(page.locator('#toast')).toContainText('Updated "test v1"');
    await expect(page.locator('#previewArticle')).toContainText('Fourth point, final.');
    await expect(docRows(page)).toHaveCount(1);
  });

  test('when both copies changed, MARS asks and suggests saving a copy', async ({ page }) => {
    const origin = `file:/tmp/both-${Date.now()}.md`;
    await page.goto(fileLink(planWith('Fourth point.'), origin));
    await expect(page.locator('#previewArticle')).toContainText('Fourth point.');
    await editSource(page, planWith('Fourth point, edited in MARS.'));
    await page.goto(fileLink(planWith('Fourth point, edited by the agent.'), origin));
    await expect(page.getByRole('heading', { name: 'Both copies changed' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Save as a copy' })).toBeFocused();
    await page.getByRole('button', { name: 'Save as a copy' }).click();
    await expect(row(page, 'Plan (new)')).toBeVisible();
    await expect(docRows(page)).toHaveCount(2);
  });

  test('changes since opened show your edits, and Unlink forgets the source', async ({ page }) => {
    const origin = `file:/tmp/opened-${Date.now()}.md`;
    await page.goto(fileLink(planWith('Fourth point.'), origin));
    await expect(page.locator('#previewArticle')).toContainText('Fourth point.');
    await editSource(page, planWith('Fourth point, edited in MARS.'));
    await page.getByRole('button', { name: 'Document options' }).click();
    await page.getByRole('button', { name: 'Show changes since opened' }).click();
    await expect(page.locator('#changesLabel')).toHaveText('1 change since opened');
    await expect(page.locator('#previewArticle .su-diff-edit')).toHaveCount(1);
    await page.getByRole('button', { name: 'Hide changes since opened' }).click();
    await expect(page.locator('#changesBar')).toBeHidden();

    await row(page, 'Plan')
      .getByRole('button', { name: /Actions for Plan/ })
      .click();
    await page.getByRole('menuitem', { name: /Unlink from source/ }).click();
    await page.getByRole('button', { name: 'Unlink', exact: true }).click();
    await expect(page.locator('#toast')).toContainText('Unlinked "Plan"');
    await page.getByRole('button', { name: 'Document options' }).click();
    await expect(page.getByRole('button', { name: 'Show changes since opened' })).toBeDisabled();
    await page.keyboard.press('Escape');

    // The source no longer maps to this document; a look-alike still asks first.
    await page.goto(fileLink(planWith('Fourth point.'), origin));
    await page.getByRole('button', { name: 'Add as new' }).click();
    await expect(docRows(page)).toHaveCount(2);
  });

  test('search matches document text, and smart search is offered at six documents', async ({ page }) => {
    await page.goto(tool.path);
    await page.getByRole('button', { name: 'Document options' }).click();
    // Chromium has linked files, so the live-reload option shows, off by default.
    await expect(page.getByLabel(/Reload linked files automatically/)).not.toBeChecked();
    await page.keyboard.press('Escape');

    const files = Array.from({ length: 6 }, (_, i) => ({
      name: `note-${i + 1}.md`,
      mimeType: 'text/markdown',
      buffer: Buffer.from(
        i === 5
          ? `# Note 6\n\nPlain words.\n${threadsBlock([{ id: 't1', quote: 'Plain words', comments: [{ id: 'c1', author: 'Oleg', at: '2026-10-21T09:00:00.000Z', body: 'Rotate the keys quarterly.' }] }])}`
          : `# Note ${i + 1}\n\n${i === 3 ? 'The cache needs a TTL before launch.' : 'Plain words.'}\n`,
      ),
    }));
    await page.locator('#importInput').setInputFiles(files);
    await expect(page.locator('#toast')).toContainText('Imported 6 documents');
    const nudge = page.locator('#smartNudge');
    await expect(nudge).toBeVisible();
    await expect(nudge).toContainText('Search by meaning');
    await nudge.getByRole('button', { name: 'Not now' }).click();
    await expect(nudge).toBeHidden();
    await page.reload();
    await expect(docRows(page)).toHaveCount(6);
    await page.waitForTimeout(2000);
    await expect(nudge).toBeHidden();

    await page.getByRole('searchbox', { name: 'Search documents' }).fill('ttl');
    await expect(docRows(page)).toHaveCount(1);
    await expect(page.locator('#tree .tree-snippet')).toHaveText('The cache needs a TTL before launch.');
    await docRows(page).click();
    await expect(page.locator('#findBar')).toBeVisible();
    await expect(page.locator('#findInput')).toHaveValue('ttl');
    await expect(page.locator('#findCount')).toHaveText('1/1');

    // Review comments are searched too.
    await page.getByRole('searchbox', { name: 'Search documents' }).fill('quarterly');
    await expect(docRows(page)).toHaveCount(1);
    await expect(page.locator('#tree .tree-snippet')).toHaveText(
      'Comment on “Plain words”: Rotate the keys quarterly.',
    );

    await page.getByRole('button', { name: 'Smart search' }).click();
    await expect(page.getByRole('heading', { name: 'Turn on smart search' })).toBeVisible();
    await expect(page.getByRole('radio')).toHaveCount(2);
    await expect(page.getByRole('radio', { name: /Best speed/ })).toBeChecked();
    await page.getByRole('button', { name: 'Cancel' }).click();
    await expect(page.getByRole('button', { name: 'Smart search' })).toHaveAttribute('aria-pressed', 'false');
  });

  for (const version of [1, 2]) {
    test(`previous UI opens database version ${version} without changing its schema or stored data`, async ({
      page,
    }) => {
      await page.route('**/rollback-storage-fixture', (route) =>
        route.fulfill({ contentType: 'text/html', body: '<!doctype html><title>Existing browser storage</title>' }),
      );
      await page.goto('/rollback-storage-fixture');
      await page.evaluate(async (version) => {
        const db = await new Promise<IDBDatabase>((resolve, reject) => {
          const request = indexedDB.open('mars', version);
          request.onupgradeneeded = () => {
            request.result.createObjectStore('nodes', { keyPath: 'id' });
            request.result.createObjectStore('docs');
            request.result.createObjectStore('meta');
            if (version === 2) request.result.createObjectStore('reviews');
          };
          request.onsuccess = () => resolve(request.result);
          request.onerror = () => reject(request.error);
        });
        const stores = version === 2 ? ['nodes', 'docs', 'meta', 'reviews'] : ['nodes', 'docs', 'meta'];
        const tx = db.transaction(stores, 'readwrite');
        tx.objectStore('nodes').put({
          id: 'existing',
          kind: 'doc',
          name: 'Existing notes',
          parentId: null,
          order: 0,
          createdAt: 1,
          updatedAt: 1,
          originId: 'original',
        });
        tx.objectStore('docs').put('# Existing notes\n\nKeep the original document.', 'existing');
        tx.objectStore('meta').put('existing', 'lastDoc');
        if (version === 2) tx.objectStore('reviews').put({ preserved: 'Review history stays untouched.' }, 'existing');
        await new Promise<void>((resolve, reject) => {
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(tx.error);
        });
        db.close();
      }, version);
      await page.goto('/');
      await expect(page.locator('#previewArticle')).toContainText('Keep the original document.');
      await expect(row(page, 'Existing notes')).toBeVisible();
      await expect(page.getByRole('button', { name: 'Review', exact: true })).toHaveCount(0);
      await page.getByRole('button', { name: 'Source', exact: true }).click();
      await setMarkdown(page, '# Existing notes\n\nEdits still save after rollback.');
      await page.getByRole('button', { name: 'Preview', exact: true }).click();
      await expect(page.locator('#saveState')).toHaveText('Saved');
      await page.reload();
      await expect(page.locator('#previewArticle')).toContainText('Edits still save after rollback.');
      const storage = await page.evaluate(async () => {
        const db = await new Promise<IDBDatabase>((resolve) => {
          const request = indexedDB.open('mars');
          request.onsuccess = () => resolve(request.result);
        });
        const result = { version: db.version, stores: [...db.objectStoreNames], history: undefined as unknown };
        if (db.objectStoreNames.contains('reviews')) {
          result.history = await new Promise((resolve) => {
            const request = db.transaction('reviews').objectStore('reviews').get('existing');
            request.onsuccess = () => resolve(request.result);
          });
        }
        db.close();
        return result;
      });
      expect(storage.version).toBe(version);
      expect(storage.stores).toEqual(version === 2 ? ['docs', 'meta', 'nodes', 'reviews'] : ['docs', 'meta', 'nodes']);
      if (version === 2) expect(storage.history).toEqual({ preserved: 'Review history stays untouched.' });
    });
  }
});
