import { expect, type Page } from '@playwright/test';

export async function copyLink(
  page: Page,
  options: {
    mode?: 'snapshot' | 'live';
    format?: 'short' | 'full';
    permission?: 'view' | 'comment' | 'edit';
    ttl?: string;
    close?: boolean;
  } = {},
) {
  await page.getByRole('button', { name: 'Share', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Copy link', exact: true }).click();
  return configureCopyLink(page, options);
}
export async function configureCopyLink(
  page: Page,
  options: {
    mode?: 'snapshot' | 'live';
    format?: 'short' | 'full';
    permission?: 'view' | 'comment' | 'edit';
    ttl?: string;
    close?: boolean;
  } = {},
) {
  const dialog = page.getByRole('dialog', { name: 'Copy link', exact: true });
  await expect(dialog).toBeVisible();
  const mode = dialog.getByRole('radio', { name: options.mode === 'live' ? 'Live' : 'Snapshot', exact: true });
  // Folder links are snapshots and have no mode selector.
  if (options.mode === 'live' || (await mode.count())) await mode.check();
  const forwarding = await dialog.locator('#copyForwardInfo').isVisible();
  if (!forwarding) {
    await dialog.getByRole('radio', { name: options.format === 'full' ? 'Full' : 'Short', exact: true }).check();
    if (options.mode === 'live')
      await dialog.getByRole('radio', { name: `Can ${options.permission ?? 'view'}`, exact: true }).check();
    if (options.ttl) await dialog.getByLabel('Expires', { exact: true }).selectOption(options.ttl);
  }
  await dialog.getByRole('button', { name: 'Copy link', exact: true }).click();
  await expect(dialog).not.toHaveAttribute('aria-busy', 'true', { timeout: 20000 });
  const input = dialog.getByRole('textbox', { name: 'Link', exact: true });
  await expect.poll(() => input.inputValue(), { timeout: 20000 }).toMatch(/^https?:\/\//);
  const value = await input.inputValue();
  if (options.close !== false) await dialog.getByRole('button', { name: 'Close', exact: true }).click();
  return value;
}
