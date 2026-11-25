import { createRequire } from 'node:module';
import { resolve } from 'node:path';

import { expect, type Page } from '@playwright/test';

import { test } from './_mars_test';

const baseURL = process.env.MARS_BASE_URL ?? 'http://localhost:7373';
test.use({ baseURL });
const require = createRequire(resolve(process.cwd(), '../dev/tools/mars/package.json'));
const { zipSync } = require('fflate') as { zipSync(files: Record<string, Uint8Array>): Uint8Array };

async function connectedStatus(page: Page, folder = false) {
  await page.goto('/');
  await expect(page.locator('#emptyState')).toBeVisible();
  if (folder) {
    await page.locator('#importInput').setInputFiles({
      name: 'Folder.zip',
      mimeType: 'application/zip',
      buffer: Buffer.from(zipSync({ 'Folder/Status.md': Buffer.from('# Status\n\nBefore.') })),
    });
    await expect(page.locator('#toast')).toContainText('Imported 1 document');
  } else {
    await page
      .locator('#fileInput')
      .setInputFiles({ name: 'Status.md', mimeType: 'text/markdown', buffer: Buffer.from('# Status\n\nBefore.') });
    await expect(page.locator('#docTitle')).toHaveText('Status');
  }
  await page.getByRole('button', { name: `Actions for ${folder ? 'Folder' : 'Status'}`, exact: true }).click();
  await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
  await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible({ timeout: 15000 });
  await page.getByRole('button', { name: 'Close', exact: true }).click();
}

async function openStatus(page: Page, name: string) {
  await page.getByRole('button', { name: `Actions for ${name}`, exact: true }).click();
  await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
  return page.getByRole('dialog', { name: 'Sync', exact: true });
}

async function editStatus(page: Page) {
  await page.getByRole('link', { name: 'Status', exact: true }).click();
  await page.getByRole('button', { name: 'Source', exact: true }).click();
  await page.waitForFunction(() => !!(window as { __suEditorAPI?: unknown }).__suEditorAPI);
  await page.evaluate(() =>
    (window as unknown as { __suEditorAPI: { setText(value: string): void } }).__suEditorAPI.setText(
      '# Status\n\nAfter.',
    ),
  );
  await page.getByRole('button', { name: 'Preview', exact: true }).click();
}

test.describe('sync user experience', () => {
  test.beforeEach(async ({ context }) => {
    const origin = new URL(baseURL).origin;
    const headers = { 'x-mars-test-client': crypto.randomUUID() };
    await context.route(`${origin}/**`, (route) =>
      route.continue({ headers: { ...route.request().headers(), ...headers } }),
    );
  });
  test('folder Sync settings keep both directions Manual across reload until Sync now', async ({ page }) => {
    await page.clock.install();
    await connectedStatus(page, true);
    const root = await openStatus(page, 'Folder');
    await root.getByRole('button', { name: 'Sync settings', exact: true }).click();
    const settings = page.getByRole('dialog', { name: 'Sync settings', exact: true });
    await expect(settings.getByLabel('Receive updates')).toHaveValue('5000');
    await expect(settings.getByLabel('Send updates')).toHaveValue('5000');
    await settings.getByRole('button', { name: 'About send timing' }).evaluate((button) => {
      (button as HTMLButtonElement).focus({ preventScroll: true });
    });
    await expect(page.getByRole('tooltip')).toContainText('after you stop editing');
    await settings.getByLabel('Receive updates').selectOption('manual');
    await settings.getByLabel('Send updates').selectOption('manual');
    await settings.getByRole('button', { name: 'Save', exact: true }).click();
    await root.getByRole('button', { name: 'Close', exact: true }).click();
    let sends = 0,
      receives = 0;
    page.on('request', (request) => {
      const path = new URL(request.url()).pathname;
      if (path === '/sync/append') sends++;
      if (path === '/sync/index') receives++;
    });
    await editStatus(page);
    await expect(page.locator('#previewArticle')).toContainText('After.');
    await page.clock.fastForward(30_000);
    await page.reload();
    await expect(page.locator('#previewArticle')).toContainText('After.');
    await page.clock.fastForward(30_000);
    expect(sends).toBe(0);
    expect(receives).toBe(0);
    await openStatus(page, 'Folder');
    await root.getByRole('button', { name: 'Sync now', exact: true }).click();
    await expect.poll(() => sends).toBeGreaterThan(0);
    await expect.poll(() => receives).toBeGreaterThan(0);
    await expect(root.locator('.sync-status')).toContainText('Up to date');
  });
  test('Back from Devices or Sync link returns to the unsaved folder Sync settings', async ({ page }) => {
    await connectedStatus(page, true);
    const root = await openStatus(page, 'Folder');
    const gear = root.getByRole('button', { name: 'Sync settings', exact: true });
    await gear.click();
    const settings = page.getByRole('dialog', { name: 'Sync settings', exact: true });
    await settings.getByLabel('Receive updates').selectOption('15000');
    await settings.getByLabel('Send updates').selectOption('manual');
    await settings.getByRole('button', { name: 'Devices', exact: true }).click();
    await page.getByRole('dialog', { name: 'Sync devices', exact: true }).getByRole('button', { name: 'Back' }).click();
    await expect(settings.getByLabel('Receive updates')).toHaveValue('15000');
    await expect(settings.getByLabel('Send updates')).toHaveValue('manual');
    await settings.getByRole('button', { name: 'Sync link', exact: true }).click();
    const link = page.getByRole('dialog', { name: 'Sync link', exact: true });
    await expect(link.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible();
    await link.getByRole('button', { name: 'Back' }).click();
    await expect(settings.getByLabel('Receive updates')).toHaveValue('15000');
    await expect(settings.getByLabel('Send updates')).toHaveValue('manual');
    await settings.getByRole('button', { name: 'Back', exact: true }).click();
    await expect(root.getByRole('button', { name: 'Sync now', exact: true })).toBeVisible();
    await gear.click();
    await expect(settings.getByLabel('Receive updates')).toHaveValue('5000');
    await expect(settings.getByLabel('Send updates')).toHaveValue('5000');
  });
  for (const selection of ['document', 'folder', 'child'] as const) {
    test(`open Sync status updates after upload without reopening or moving focus (${selection})`, async ({ page }) => {
      await connectedStatus(page, selection !== 'document');
      let release = () => {};
      const held = new Promise<void>((resolve) => {
        release = resolve;
      });
      await page.route('**/sync/append', async (route) => {
        await held;
        await route.fallback();
      });
      try {
        await editStatus(page);
        await expect(page.locator('#tree [data-phase="working"]')).toHaveCount(1, { timeout: 10_000 });
        const dialog = await openStatus(page, selection === 'folder' ? 'Folder' : 'Status');
        const status = dialog.locator('.sync-status');
        await expect(status).toHaveText(selection === 'child' ? 'Syncing' : 'Syncing changes…');
        await expect(status).toHaveAttribute('role', 'status');
        const control = dialog.getByRole('button', {
          name:
            selection === 'child'
              ? 'Manage folder Sync'
              : selection === 'folder'
                ? 'Sync settings'
                : 'Disconnect this device',
          exact: true,
        });
        await control.focus();
        await control.evaluate((element) => element.setAttribute('data-live-test', 'retained'));
        release();
        await expect(page.locator('#tree [data-phase="current"]')).toHaveCount(1);
        await expect(status).toHaveText('Up to date');
        await expect(control).toBeFocused();
        await expect(control).toHaveAttribute('data-live-test', 'retained');
        await expect(status.locator('.su-spinner')).toHaveCount(0);
        await page.getByRole('button', { name: 'Close', exact: true }).click();
        await expect(page.locator('#syncDialog')).toHaveCount(0);
      } finally {
        release();
      }
    });
  }

  test('open Sync status follows offline and online changes without resetting focus', async ({ page, context }) => {
    await connectedStatus(page);
    const dialog = await openStatus(page, 'Status');
    const control = dialog.getByRole('button', { name: 'Disconnect this device', exact: true });
    await control.focus();
    try {
      await context.setOffline(true);
      await expect(dialog.locator('.sync-status')).toHaveText('Offline');
      await expect(dialog.locator('[data-sync-field="message"]')).toContainText('back online');
      await expect(control).toBeFocused();
      await context.setOffline(false);
      await expect(dialog.locator('.sync-status')).toHaveText('Up to date', { timeout: 15000 });
      await expect(dialog.locator('[data-sync-field="message"]')).toBeHidden();
      await expect(control).toBeFocused();
      await dialog.getByRole('button', { name: 'Close', exact: true }).click();
    } finally {
      await context.setOffline(false);
    }
  });

  for (const outcome of ['attention', 'ended'] as const) {
    test(`open Sync status follows an upload failure in place (${outcome})`, async ({ page }) => {
      await connectedStatus(page);
      let release = () => {};
      const held = new Promise<void>((resolve) => {
        release = resolve;
      });
      await page.route('**/sync/append', async (route) => {
        await held;
        await route.fulfill({
          status: outcome === 'ended' ? 403 : 507,
          json: { error: outcome === 'ended' ? 'access-ended' : 'sync-storage-full' },
        });
      });
      try {
        await editStatus(page);
        await expect(page.locator('#tree [data-phase="working"]')).toHaveCount(1);
        const dialog = await openStatus(page, 'Status');
        const close = dialog.getByRole('button', { name: 'Close', exact: true });
        await close.focus();
        release();
        await expect(dialog.locator('.sync-status')).toHaveText(
          outcome === 'ended' ? 'Local copy · Sync ended' : 'Sync needs attention',
        );
        await expect(close).toBeFocused();
        if (outcome === 'ended') {
          await expect(dialog.getByRole('button', { name: 'Devices', exact: true })).toHaveCount(0);
          await expect(dialog.getByRole('button', { name: 'Keep as local document', exact: true })).toBeVisible();
          await expect(dialog.getByRole('button', { name: 'Sync link', exact: true })).toHaveCount(0);
        } else {
          await expect(dialog.getByRole('alert')).toContainText('Sync storage is full');
          await expect(dialog.getByRole('button', { name: 'Sync now', exact: true })).toBeVisible();
        }
        await close.click();
        await expect(page.locator('#previewArticle')).toContainText('After.');
      } finally {
        release();
      }
    });
  }

  test('open Sync status shows enrollment errors immediately without resetting focus', async ({ page }) => {
    await connectedStatus(page, true);
    let release = () => {};
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route('**/sync/document', async (route) => {
      await held;
      await route.fulfill({ status: 507, json: { error: 'document-limit' } });
    });
    try {
      await page.getByRole('button', { name: 'Actions for Folder', exact: true }).click();
      const choosing = page.waitForEvent('filechooser');
      await page.getByRole('menuitem', { name: 'Import files…', exact: true }).click();
      await (
        await choosing
      ).setFiles({ name: 'Pending.md', mimeType: 'text/markdown', buffer: Buffer.from('# Pending\n\nKeep this.') });
      await expect(page.locator('#toast')).toContainText('Imported 1 document');
      const dialog = await openStatus(page, 'Folder');
      await expect(dialog.locator('.sync-status')).toHaveText('Waiting to sync');
      await expect(dialog.getByRole('alert')).toHaveCount(0);
      const settings = dialog.getByRole('button', { name: 'Sync settings', exact: true });
      await settings.focus();
      release();
      await expect(dialog.locator('.sync-status')).toHaveText('Sync needs attention', { timeout: 10_000 });
      await expect(dialog.getByRole('alert')).toContainText('1,000');
      await expect(dialog.getByRole('alert')).toContainText('saved locally');
      await expect(settings).toBeFocused();
      await expect(dialog.getByRole('button', { name: 'Sync now', exact: true })).toBeVisible();
      await page.getByRole('button', { name: 'Close', exact: true }).click();
    } finally {
      release();
    }
  });

  test('open Sync status keeps Sync now focused while pending enrollment completes', async ({ page }) => {
    await connectedStatus(page, true);
    let release = () => {};
    const held = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route('**/sync/document', async (route) => {
      await held;
      await route.fallback();
    });
    try {
      await page.getByRole('button', { name: 'Actions for Folder', exact: true }).click();
      const choosing = page.waitForEvent('filechooser');
      await page.getByRole('menuitem', { name: 'Import files…', exact: true }).click();
      await (
        await choosing
      ).setFiles({ name: 'Pending.md', mimeType: 'text/markdown', buffer: Buffer.from('# Pending\n\nKeep this.') });
      await expect(page.locator('#toast')).toContainText('Imported 1 document');
      const dialog = await openStatus(page, 'Folder');
      await expect(dialog.locator('[data-sync-field="pending"]')).toContainText('1 item is saved locally');
      const syncNow = dialog.locator('footer').getByRole('button', { name: 'Sync now', exact: true });
      await syncNow.focus();
      release();
      await expect(dialog.locator('.sync-status')).toHaveText('Up to date', { timeout: 15000 });
      await expect(dialog.locator('[data-sync-field="pending"]')).toBeHidden();
      await expect(syncNow).toBeFocused();
      await expect(syncNow).toBeEnabled();
      await dialog.getByRole('button', { name: 'Close', exact: true }).click();
    } finally {
      release();
    }
  });
  test('sidebar Sync icons complete full turns across repeated renders and respect reduced motion', async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await page.goto('/');
    await expect(page.locator('#emptyState')).toBeVisible();
    await page.getByRole('button', { name: 'New document, folder, or import', exact: true }).click();
    await page.getByRole('menuitem', { name: 'New folder', exact: true }).click();
    await page.getByRole('textbox', { name: 'Name', exact: true }).fill('Rotating folder');
    await page.getByRole('textbox', { name: 'Name', exact: true }).press('Enter');
    await page.locator('#fileInput').setInputFiles({
      name: 'Rotating document.md',
      mimeType: 'text/markdown',
      buffer: Buffer.from('# Animation fixture'),
    });
    let release!: () => void;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route('**/sync/append', async (route) => {
      await gate;
      await route.fallback();
    });
    try {
      await page.getByRole('button', { name: 'Actions for Rotating folder', exact: true }).click();
      await page.getByRole('menuitem', { name: 'Sync', exact: true }).click();
      await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
      await expect(page.locator('#tree .tree-connection[data-phase="working"]')).toHaveCount(1);
      const samples = await page.evaluate(async () => {
        const history: { time: number; angle: number }[][] = [[]],
          started = performance.now();
        let nextRender = started;
        while (performance.now() - started < 1300) {
          await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
          if (performance.now() >= nextRender) {
            document.querySelector('#sbFilter')!.dispatchEvent(new Event('input', { bubbles: true }));
            nextRender = performance.now() + 50;
          }
          [...document.querySelectorAll('#tree .tree-connection svg')].forEach((icon, index) => {
            const animation = icon
              .getAnimations()
              .find((animation) => animation instanceof CSSAnimation && animation.animationName === 'sync-turn');
            const transform = new DOMMatrixReadOnly(getComputedStyle(icon).transform),
              angle = ((Math.atan2(transform.b, transform.a) * 180) / Math.PI + 360) % 360;
            history[index].push({ time: Number(animation?.currentTime), angle });
          });
        }
        return history;
      });
      for (const history of samples) {
        expect(history.at(-1)!.time - history[0].time).toBeGreaterThan(1100);
        expect(new Set(history.map((sample) => Math.floor(sample.angle / 90))).size).toBe(4);
        expect(history.slice(1).every((sample, index) => sample.time >= history[index].time)).toBe(true);
      }
      await page.emulateMedia({ reducedMotion: 'reduce' });
      for (const icon of await page.locator('#tree .tree-connection svg').all())
        await expect(icon).toHaveCSS('animation-name', 'none');
      await page.getByRole('searchbox', { name: 'Search documents', exact: true }).dispatchEvent('input');
      expect(
        await page
          .locator('#tree .tree-connection svg')
          .evaluateAll((icons) => icons.flatMap((icon) => icon.getAnimations()).length),
      ).toBe(0);
    } finally {
      release();
    }
  });

  test('Sync loading uses a spinner and Copied feedback never moves the dialog', async ({ page, context }) => {
    await context.addInitScript(() =>
      Object.defineProperty(navigator, 'clipboard', {
        value: { write: async () => {}, writeText: async () => {} },
        configurable: true,
      }),
    );
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    let release = () => {};
    const waiting = new Promise<void>((resolve) => {
      release = resolve;
    });
    await page.route('**/sync/create', async (route) => {
      await waiting;
      await route.fallback();
    });
    try {
      await page.goto('/');
      await expect(page.locator('#emptyState')).toBeVisible();
      await page.locator('#fileInput').setInputFiles({
        name: 'Report.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('# Report\n\nA saved document.'),
      });
      await expect(page.locator('#docTitle')).toHaveText('Report');
      await page.getByRole('button', { name: 'Actions for Report', exact: true }).click();
      await page.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
      await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
      const dialog = page.locator('#syncDialog'),
        loading = dialog.getByRole('button', { name: /Turning on Sync/ });
      await expect(loading).toBeDisabled();
      await expect(loading.locator('.su-spinner')).toBeVisible();
      await expect(loading.locator('.su-spinner')).toHaveCSS('animation-name', 'su-spinner-turn');
      await expect(dialog).toHaveAttribute('aria-busy', 'true');
      release();
      await expect(dialog.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible({ timeout: 15000 });
      await expect(dialog).toContainText(
        'The link expires in 10 minutes. Keep this window open until your other device connects.',
      );
      const before = (await dialog.boundingBox())!;
      await dialog.getByRole('button', { name: 'Copy link', exact: true }).click();
      await expect(page.getByRole('tooltip')).toHaveText('Copied');
      const after = (await dialog.boundingBox())!;
      expect(after).toEqual(before);
      await expect(dialog.locator('.sync-notice')).toHaveText('');
    } finally {
      release();
    }
  });
  test('Live links use spaced access controls, simple copy, and stable clipboard feedback', async ({
    page,
    context,
  }) => {
    await context.addInitScript(() =>
      Object.defineProperty(navigator, 'clipboard', {
        value: { write: async () => {}, writeText: async () => {} },
        configurable: true,
      }),
    );
    await page.goto('/');
    await expect(page.locator('#emptyState')).toBeVisible();
    await page.locator('#fileInput').setInputFiles({
      name: 'Report.md',
      mimeType: 'text/markdown',
      buffer: Buffer.from('# Report\n\nA saved document.'),
    });
    await expect(page.locator('#docTitle')).toHaveText('Report');
    await page.getByRole('button', { name: 'Share', exact: true }).click();
    await page.getByRole('menuitem', { name: 'Copy link', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Copy link', exact: true });
    await dialog.getByRole('radio', { name: 'Live', exact: true }).check();
    await expect(dialog.getByRole('radio', { name: 'Full', exact: true })).toBeChecked();
    await dialog.getByRole('radio', { name: 'Short', exact: true }).check();
    await expect(dialog.locator('#copyAccessFields input[type="radio"]')).toHaveCount(3);
    await expect(dialog.getByLabel('Expires', { exact: true })).toHaveValue('604800');
    expect(
      await dialog
        .getByLabel('Expires', { exact: true })
        .locator('option')
        .evaluateAll((options) => options.map((option) => (option as HTMLOptionElement).value)),
    ).toEqual(['86400', '604800', '1209600', '2592000']);
    await expect(dialog).not.toContainText(/fragment|reusable|remote storage/i);
    await dialog.getByRole('radio', { name: 'Can comment', exact: true }).check();
    await dialog.getByLabel('Expires', { exact: true }).selectOption('2592000');
    const rows = await dialog
      .locator('.live-access label')
      .evaluateAll((labels) => labels.map((label) => label.getBoundingClientRect().toJSON()));
    expect(rows[1].y).toBeGreaterThan(rows[0].y + rows[0].height);
    await dialog.getByRole('button', { name: 'Copy link', exact: true }).click();
    await expect.poll(() => dialog.getByRole('textbox', { name: 'Link', exact: true }).inputValue()).toMatch(/^http/);
    expect(new URL(await dialog.getByRole('textbox', { name: 'Link', exact: true }).inputValue()).hash).toMatch(
      /^#live=[A-Za-z0-9_-]{22}$/,
    );
    await expect(dialog.getByRole('radio', { name: 'Can comment', exact: true })).toBeChecked();
    const before = (await dialog.boundingBox())!;
    await dialog.getByRole('button', { name: 'Copy link', exact: true }).click();
    await expect(page.getByRole('tooltip')).toHaveText('Copied');
    expect(await dialog.boundingBox()).toEqual(before);
    await expect(dialog.getByRole('radio', { name: 'Can comment', exact: true })).toBeChecked();
    await expect(dialog.getByLabel('Expires', { exact: true })).toHaveValue('2592000');
    await dialog.getByRole('button', { name: 'Close', exact: true }).click();
    await page.getByRole('button', { name: 'Document options', exact: true }).click();
    const sharedDefault = page.getByRole('button', { name: 'Set shared default', exact: true });
    await expect(sharedDefault).toBeVisible();
    await expect(sharedDefault).toHaveCSS('margin-top', '12px');
    const fieldBox = (await page
        .getByRole('dialog', { name: 'Document options', exact: true })
        .getByLabel('Reading width', { exact: true })
        .boundingBox())!,
      actionBox = (await sharedDefault.boundingBox())!;
    expect(actionBox.y - fieldBox.y - fieldBox.height).toBeGreaterThanOrEqual(12);
  });
  for (const [width, theme] of [
    [320, 'dark'],
    [1280, 'light'],
  ] as const) {
    test(`tooltip labels and arrows fit viewport edges at ${width}px ${theme}`, async ({ page }) => {
      await page.setViewportSize({ width, height: 800 });
      await page.goto('/');
      await expect(page.locator('#emptyState')).toBeVisible();
      await page.evaluate((theme) => {
        document.documentElement.dataset.theme = theme;
      }, theme);
      for (const [label, position] of [
        ['Up to date', 'top: 2px; right: 2px;'],
        ['Disconnect device', 'bottom: 2px; right: 2px;'],
        ['A'.repeat(180), 'bottom: 2px; left: 2px;'],
      ] as const) {
        await page.evaluate(
          ({ label, position }) => {
            document.querySelector('#tooltipProbe')?.remove();
            const button = document.createElement('button');
            button.id = 'tooltipProbe';
            button.className = 'icon-btn';
            button.title = label;
            button.setAttribute('aria-label', label);
            button.textContent = '?';
            button.style.cssText = `position: fixed; z-index: 1000; ${position}`;
            document.body.append(button);
          },
          { label, position },
        );
        const button = page.getByRole('button', { name: label, exact: true });
        await expect(button).toHaveAttribute('data-tooltip', label);
        await button.focus();
        const tip = page.locator('.su-tooltip');
        await expect(tip).toHaveText(label);
        const box = (await tip.boundingBox())!;
        expect(box.x).toBeGreaterThanOrEqual(7);
        expect(box.x + box.width).toBeLessThanOrEqual(width - 7);
        expect(box.y).toBeGreaterThanOrEqual(7);
        expect(box.y + box.height).toBeLessThanOrEqual(793);
        if (label.length < 30) {
          expect(box.height).toBeLessThanOrEqual(36);
          expect(box.width).toBeGreaterThan(70);
        }
        await expect(tip).toHaveCSS('overflow', 'visible');
        expect(await tip.evaluate((el) => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
        await page.mouse.move(width / 2, 400);
        await button.focus();
        await expect(tip).toHaveText(label);
        await page.keyboard.press('Escape');
        await expect(tip).toHaveCount(0);
      }
    });
  }
  for (const width of [320, 760]) {
    for (const theme of ['light', 'dark']) {
      test.describe(`Sync footer ${width}px ${theme}`, () => {
        test.use({ viewport: { width, height: 820 }, hasTouch: width === 320 });
        test('keeps Back and Disconnect in a compact footer without a Backup section', async ({ page }) => {
          await page.goto('/');
          await expect(page.locator('#emptyState')).toBeVisible();
          await page.evaluate((theme) => (document.documentElement.dataset.theme = theme), theme);
          await page.locator('#fileInput').setInputFiles({
            name: 'Report.md',
            mimeType: 'text/markdown',
            buffer: Buffer.from('# Report\n\nA saved document.'),
          });
          await expect(page.locator('#docTitle')).toHaveText('Report');
          if (!(await page.getByRole('button', { name: 'Actions for Report', exact: true }).isVisible()))
            await page.getByRole('button', { name: 'Toggle documents sidebar', exact: true }).click();
          await page.getByRole('button', { name: 'Actions for Report', exact: true }).click();
          await page.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
          await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
          const dialog = page.locator('#syncDialog'),
            footer = dialog.locator('footer');
          const settings = page.getByRole('dialog', { name: 'Sync settings', exact: true });
          await expect(dialog.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible({ timeout: 15000 });
          const back = (await footer.getByRole('button', { name: 'Back', exact: true }).boundingBox())!,
            copy = (await footer.getByRole('button', { name: 'Copy link', exact: true }).boundingBox())!;
          expect(back.x).toBeLessThan(copy.x);
          expect(Math.abs(back.y - copy.y)).toBeLessThanOrEqual(1);
          await expect(dialog.locator('header').getByRole('button', { name: 'Back', exact: true })).toHaveCount(0);
          await footer.getByRole('button', { name: 'Back', exact: true }).click();
          await settings.getByRole('button', { name: 'Close', exact: true }).click();
          await expect(footer.getByRole('button', { name: 'Sync now', exact: true })).toBeVisible();
          const stop = (await footer
              .getByRole('button', { name: 'Disconnect this device', exact: true })
              .boundingBox())!,
            syncNow = (await footer.getByRole('button', { name: 'Sync now', exact: true }).boundingBox())!;
          expect(stop.x).toBeLessThan(syncNow.x);
          if (width >= 760) expect(Math.abs(stop.y - syncNow.y)).toBeLessThanOrEqual(1);
          else {
            expect(syncNow.y - stop.y).toBeGreaterThanOrEqual(0);
            expect(syncNow.y - stop.y).toBeLessThanOrEqual(40);
          }
          const bounds = (await dialog.boundingBox())!;
          for (const control of [stop, syncNow]) {
            expect(control.x).toBeGreaterThanOrEqual(bounds.x);
            expect(control.x + control.width).toBeLessThanOrEqual(bounds.x + bounds.width);
          }
          await expect(dialog.locator('details')).toHaveCount(0);
          const header = dialog.locator('header');
          const gear = header.getByRole('button', { name: 'Sync settings', exact: true });
          expect((await gear.boundingBox())!.x).toBeLessThan(
            (await header.getByRole('button', { name: 'Close', exact: true }).boundingBox())!.x,
          );
          await gear.click();
          await expect(settings.getByRole('button', { name: 'Devices', exact: true })).toBeVisible();
          await expect(settings.getByRole('button', { name: 'Sync link', exact: true })).toBeVisible();
          await settings.getByRole('button', { name: 'Close', exact: true }).click();
          await expect(dialog.getByRole('button', { name: 'Backup', exact: true })).toHaveCount(0);
          expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
          await dialog.getByRole('button', { name: 'Close', exact: true }).click();
          if (await page.locator('#sbBackdrop').isVisible())
            await page.getByRole('button', { name: 'Toggle documents sidebar', exact: true }).click();
          await page.getByRole('button', { name: 'Share', exact: true }).click();
          await page.getByRole('menuitem', { name: 'Copy link', exact: true }).click();
          const liveDialog = page.getByRole('dialog', { name: 'Copy link', exact: true });
          await liveDialog.getByRole('radio', { name: 'Live', exact: true }).check();
          await expect(liveDialog.getByRole('radio', { name: 'Can view', exact: true })).toBeChecked();
          await expect(liveDialog.getByLabel('Expires', { exact: true })).toBeVisible();
          expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
        });
      });
    }
  }
  test('the hourly connection limit has a clear error and keeps the document local', async ({ page }) => {
    await page.route('**/sync/create', (route) =>
      route.fulfill({
        status: 429,
        contentType: 'application/json',
        body: JSON.stringify({ error: 'collection-creation-limit' }),
      }),
    );
    await page.goto('/');
    await expect(page.locator('#emptyState')).toBeVisible();
    await page
      .locator('#fileInput')
      .setInputFiles({ name: 'Report.md', mimeType: 'text/markdown', buffer: Buffer.from('# Report\n\nMy document.') });
    await expect(page.locator('#docTitle')).toHaveText('Report');
    await page.getByRole('button', { name: 'Actions for Report', exact: true }).click();
    await page.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
    await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
    await expect(page.locator('#syncDialog').getByRole('alert')).toContainText(
      'Too many new sync connections from this network. Try again later. Your documents stay here.',
    );
    await expect(page.locator('#tree [data-connection]')).toHaveCount(0);
    await page.getByRole('button', { name: 'Close', exact: true }).click();
    await page.reload();
    await expect(page.locator('#previewArticle')).toContainText('My document.');
    await expect(page.locator('#tree [data-connection]')).toHaveCount(0);
  });
  test('Sync link errors keep Back and Close available, and Sync can reopen immediately', async ({ page }) => {
    await page.route('**/sync/pair-pending', (route) =>
      route.fulfill({
        status: 410,
        contentType: 'application/json',
        body: JSON.stringify({ error: 'invitation-expired' }),
      }),
    );
    await page.goto('/');
    await expect(page.locator('#emptyState')).toBeVisible();
    await page
      .locator('#fileInput')
      .setInputFiles({ name: 'Report.md', mimeType: 'text/markdown', buffer: Buffer.from('# Report\n\nMy document.') });
    await expect(page.locator('#docTitle')).toHaveText('Report');
    await page.getByRole('button', { name: 'Actions for Report', exact: true }).click();
    await page.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
    await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
    await expect(page.locator('#syncDialog').getByRole('alert')).toHaveText(
      'This Sync link expired. Go back and create a new one.',
      { timeout: 15000 },
    );
    await page.getByRole('button', { name: 'Back', exact: true }).click();
    await page
      .getByRole('dialog', { name: 'Sync settings', exact: true })
      .getByRole('button', { name: 'Close' })
      .click();
    await expect(page.getByRole('dialog', { name: /^(Sync|Connections)$/, exact: true })).toBeVisible();
    await page.unroute('**/sync/pair-pending');
    for (let i = 0; i < 3; i++) {
      await page.getByRole('button', { name: 'Close', exact: true }).click();
      await page.getByRole('button', { name: 'Actions for Report', exact: true }).click();
      await page.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
      await expect(page.getByRole('dialog', { name: /^(Sync|Connections)$/, exact: true })).toHaveCount(1);
    }
    await page.getByRole('button', { name: 'Sync settings', exact: true }).click();
    await page.getByRole('button', { name: 'Sync link', exact: true }).click();
    await expect(page.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.locator('#syncDialog')).toHaveCount(0);
    await expect(page.locator('#previewArticle')).toContainText('My document.');
  });
  test('sync setup, links, access, and disconnection stay in one plain-language dialog', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#emptyState')).toBeVisible();
    await page
      .locator('#fileInput')
      .setInputFiles({ name: 'Report.md', mimeType: 'text/markdown', buffer: Buffer.from('# Report\n\nMy document.') });
    await expect(page.locator('#docTitle')).toHaveText('Report');
    await page.getByRole('button', { name: 'Actions for Report', exact: true }).click();
    await expect(page.getByRole('menuitem', { name: 'Sync link', exact: true })).toHaveCount(0);
    await page.getByRole('menuitem', { name: /^(Sync|Connections)$/, exact: true }).click();
    const dialog = page.locator('#syncDialog');
    await expect(dialog).toContainText('Only this document is included.');
    await expect(dialog).not.toContainText('document(s)');
    await page.evaluate(() => {
      (window as unknown as { __syncDialog: Element | null }).__syncDialog = document.getElementById('syncDialog');
    });
    await page.getByRole('button', { name: 'Turn on sync', exact: true }).click();
    await expect(page.getByRole('textbox', { name: 'Sync link', exact: true })).toBeVisible({ timeout: 15000 });
    await expect(dialog).not.toContainText(/invite|invitation/i);
    await expect(dialog.getByRole('textbox', { name: 'Sync link', exact: true })).toHaveValue(/#sync=/);
    await expect(dialog.locator('header').getByRole('button', { name: 'Back', exact: true })).toHaveCount(0);
    await expect(dialog.locator('footer').getByRole('button', { name: 'Back', exact: true })).toBeVisible();
    expect(
      await page.evaluate(
        () =>
          (window as unknown as { __syncDialog: Element | null }).__syncDialog ===
          document.getElementById('syncDialog'),
      ),
    ).toBe(true);
    await page.getByRole('button', { name: 'Back', exact: true }).click();
    const settings = page.getByRole('dialog', { name: 'Sync settings', exact: true });
    await settings.getByRole('button', { name: 'Close', exact: true }).click();
    await expect(page.getByRole('dialog', { name: /^(Sync|Connections)$/, exact: true })).toBeVisible();
    await expect(dialog.locator('footer').getByRole('button')).toHaveCount(2);
    await expect(
      dialog.locator('footer').getByRole('button', { name: 'Disconnect this device', exact: true }),
    ).toBeVisible();
    await expect(
      dialog.locator('nav').getByRole('button', { name: 'Disconnect this device', exact: true }),
    ).toHaveCount(0);
    await expect(dialog).not.toContainText('polling');
    await expect(dialog).not.toContainText('lease');
    await page.getByRole('button', { name: 'Sync settings', exact: true }).click();
    await page.getByRole('button', { name: 'Devices', exact: true }).click();
    await expect(dialog).toContainText('No other devices connected yet.');
    await expect(dialog.getByRole('button', { name: /^(Disconnect|Disable)/ })).toHaveCount(0);
    await page.getByRole('button', { name: 'Back', exact: true }).click();
    await settings.getByRole('button', { name: 'Close', exact: true }).click();
    await expect(dialog.locator('details')).toHaveCount(0);
    await expect(dialog.getByRole('button', { name: 'Backup', exact: true })).toHaveCount(0);
    await page.getByRole('button', { name: 'Disconnect this device', exact: true }).click();
    await expect(dialog).toContainText('Other devices and active Live links remain connected.');
    await page.getByRole('button', { name: 'Disconnect', exact: true }).click();
    await expect(dialog).toContainText('Sync is off.');
    await expect(page.locator('#tree [data-connection]')).toHaveCount(0);
    await page.getByRole('button', { name: 'Close', exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await expect(page.locator('#previewArticle')).toContainText('My document.');
  });
  test('table-cell selection formatting stays in the cell rather than matching a header URL', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('#emptyState')).toBeVisible();
    const text =
      '# Cells\n\n| [Header](https://example.test/Value/alpha/text/cell/words) | Value |\n| --- | --- |\n| alpha text | cell words |\n';
    await page
      .locator('#fileInput')
      .setInputFiles({ name: 'Cells.md', mimeType: 'text/markdown', buffer: Buffer.from(text) });
    const cell = page.locator('#previewArticle tbody td').first();
    await expect(cell).toHaveText('alpha text');
    await page.getByRole('button', { name: 'Source', exact: true }).click();
    await expect(page.getByRole('textbox', { name: 'Markdown source', exact: true })).toBeVisible();
    await page.getByRole('button', { name: 'Preview', exact: true }).click();
    await expect(cell).toHaveText('alpha text');
    await cell.evaluate((element) => {
      const text = element.firstChild!;
      const range = document.createRange();
      range.setStart(text, 0);
      range.setEnd(text, 5);
      getSelection()!.removeAllRanges();
      getSelection()!.addRange(range);
      element.dispatchEvent(new Event('contextmenu', { bubbles: true }));
    });
    await page
      .getByRole('toolbar', { name: 'Format selection' })
      .getByRole('button', { name: 'Bold', exact: true })
      .click();
    await expect(cell.locator('strong')).toHaveText('alpha');
    await expect(page.locator('#previewArticle thead a')).toHaveAttribute(
      'href',
      'https://example.test/Value/alpha/text/cell/words',
    );
    await page
      .getByRole('toolbar', { name: 'Format selection' })
      .getByRole('button', { name: 'Italic', exact: true })
      .click();
    await expect(cell.locator('em strong, strong em')).toHaveText('alpha');
    const header = page.locator('#previewArticle thead th').nth(1);
    await header.evaluate((element) => {
      const range = document.createRange();
      range.selectNodeContents(element);
      getSelection()!.removeAllRanges();
      getSelection()!.addRange(range);
      element.dispatchEvent(new Event('contextmenu', { bubbles: true }));
    });
    await page
      .getByRole('toolbar', { name: 'Format selection' })
      .getByRole('button', { name: 'Bold', exact: true })
      .click();
    await expect(header.locator('strong')).toHaveText('Value');
    await expect(cell.locator('em strong, strong em')).toHaveText('alpha');
    await page.locator('#previewArticle tbody tr').evaluate((row) => {
      const range = document.createRange();
      range.setStart(row.firstChild!, 0);
      range.setEnd(row.lastChild!, row.lastChild!.childNodes.length);
      getSelection()!.removeAllRanges();
      getSelection()!.addRange(range);
      row.dispatchEvent(new Event('contextmenu', { bubbles: true }));
    });
    await expect(
      page.getByRole('toolbar', { name: 'Format selection' }).getByRole('button', { name: 'Bold', exact: true }),
    ).toBeHidden();
    await expect(page.locator('#previewArticle thead a')).toHaveAttribute(
      'href',
      'https://example.test/Value/alpha/text/cell/words',
    );
  });
});
