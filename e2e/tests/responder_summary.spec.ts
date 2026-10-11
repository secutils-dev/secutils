import type { Request } from '@playwright/test';
import { expect, test } from '@playwright/test';

import { dismissAllToasts, ensureUserAndLogin, listEndpointMatcher } from '../helpers';

const LARGE_BODY = `LARGE-BODY-MARKER\n${'0123456789abcdef'.repeat(64).concat('\n').repeat(1024)}`;
const LARGE_SCRIPT = '// LARGE-SCRIPT-MARKER\nreturn { body: context.body };';

test.describe('Responder summaries', () => {
  test('grid loads summaries and editors load full responders on demand', async ({ request, page }) => {
    await ensureUserAndLogin(request, page);

    const secretRes = await page.request.post('/api/user/secrets', { data: { name: 'LARGE_KEY', value: 'value' } });
    expect(secretRes.ok()).toBeTruthy();
    const tagRes = await page.request.post('/api/user/tags', { data: { name: 'heavy', color: '#54B399' } });
    expect(tagRes.ok()).toBeTruthy();
    const tag = await tagRes.json();

    const largeRes = await page.request.post('/api/webhooks/responders', {
      data: {
        name: 'large',
        location: { pathType: '=', path: '/large' },
        method: 'POST',
        enabled: true,
        settings: {
          requestsToTrack: 5,
          statusCode: 200,
          headers: [['X-Marker', 'large']],
          body: LARGE_BODY,
          script: LARGE_SCRIPT,
          secrets: { type: 'selected', secrets: ['LARGE_KEY'] },
          notifications: { throttleSeconds: 3600 },
        },
        tagIds: [tag.id],
      },
    });
    expect(largeRes.ok()).toBeTruthy();
    const large = await largeRes.json();

    const smallRes = await page.request.post('/api/webhooks/responders', {
      data: {
        name: 'small',
        location: { pathType: '=', path: '/small' },
        method: 'ANY',
        enabled: true,
        settings: { requestsToTrack: 0, statusCode: 200, body: 'small' },
      },
    });
    expect(smallRes.ok()).toBeTruthy();

    await page.goto('/ws/webhooks__responders');
    const largeRow = page.getByRole('row').filter({ has: page.getByRole('link', { name: 'large', exact: true }) });
    const smallRow = page.getByRole('row').filter({ has: page.getByRole('link', { name: 'small', exact: true }) });
    await expect(largeRow).toBeVisible({ timeout: 15000 });
    await expect(largeRow.getByText('1.0 MB')).toBeVisible();

    // Saving a responder refreshes the grid with a single summary request: no stats or detail
    // requests for the other (large) responder.
    await smallRow.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit responder' }) });
    await expect(flyout).toBeVisible();

    const responderGets: string[] = [];
    const collect = (req: Request) => {
      if (req.method() === 'GET' && new URL(req.url()).pathname.startsWith('/api/webhooks/responders')) {
        responderGets.push(new URL(req.url()).pathname);
      }
    };
    page.on('request', collect);
    const refresh = page.waitForResponse(
      (res) => res.request().method() === 'GET' && listEndpointMatcher('/api/webhooks/responders').test(res.url()),
    );
    await flyout.getByLabel('Status code').fill('201');
    await flyout.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('Successfully updated "small" responder')).toBeVisible({ timeout: 15000 });
    await refresh;
    await page.waitForLoadState('networkidle');
    page.off('request', collect);
    expect(responderGets).toEqual(['/api/webhooks/responders']);
    await dismissAllToasts(page);

    // Edit receives the complete responder.
    await largeRow.getByRole('button', { name: 'Edit' }).click();
    await expect(flyout).toBeVisible({ timeout: 15000 });
    await expect(flyout.getByText('LARGE-BODY-MARKER')).toBeVisible();
    await expect(flyout.getByText('X-Marker: large')).toBeVisible();
    await expect(flyout.getByText('LARGE-SCRIPT-MARKER')).toBeVisible();
    await expect(flyout.getByRole('button', { name: 'Save' })).toBeDisabled();
    await flyout.getByRole('button', { name: 'Close' }).click();
    await expect(flyout).toBeHidden();

    // Duplicate keeps every setting and tag of the source responder.
    await largeRow.getByRole('button', { name: 'All actions, row' }).click();
    await page.getByRole('menuitem', { name: 'Duplicate', exact: true }).click();
    await expect(flyout).toBeVisible({ timeout: 15000 });
    await expect(flyout.getByText('LARGE-BODY-MARKER')).toBeVisible();
    await flyout.getByLabel('Path', { exact: true }).fill('/large-copy');
    await flyout.getByRole('button', { name: 'Save' }).click();
    await expect(page.getByText('Successfully saved "large (Copy 1)" responder')).toBeVisible({ timeout: 15000 });

    const listRes = await page.request.get('/api/webhooks/responders?q=large%20(Copy');
    const { items } = await listRes.json();
    expect(items).toHaveLength(1);
    const copyRes = await page.request.get(`/api/webhooks/responders/${items[0].id}`);
    expect(copyRes.ok()).toBeTruthy();
    const copy = await copyRes.json();
    expect(copy.name).toBe('large (Copy 1)');
    expect(copy.location).toEqual({ pathType: '=', path: '/large-copy' });
    expect(copy.method).toBe(large.method);
    expect(copy.settings).toEqual(large.settings);
    expect(copy.tags.map((t: { id: string }) => t.id)).toEqual([tag.id]);
  });
});
