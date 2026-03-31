import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

test.describe('Responder HAR Export', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('exports tracked requests as a valid HAR file', async ({ page }) => {
    const stateResponse = await page.request.get('/api/ui/state');
    const state = await stateResponse.json();
    const userHandle = state.user.handle;

    const createResponse = await page.request.post('/api/webhooks/responders', {
      data: {
        name: 'har-test',
        location: { pathType: '=', path: '/har-test', subdomainPrefix: null },
        method: 'ANY',
        enabled: true,
        settings: {
          requestsToTrack: 10,
          statusCode: 200,
          body: 'hello-har',
          headers: [['content-type', 'text/plain']],
        },
      },
    });
    expect(createResponse.ok()).toBeTruthy();
    const responder = await createResponse.json();

    const webhookUrl = `/api/webhooks/${userHandle}/har-test`;
    const reqOne = await page.request.fetch(webhookUrl, { method: 'GET' });
    expect(reqOne.ok()).toBeTruthy();
    const reqTwo = await page.request.fetch(webhookUrl, {
      method: 'POST',
      data: 'test-body',
    });
    expect(reqTwo.ok()).toBeTruthy();

    await page.goto(`/ws/webhooks__responders?q=${responder.id}`);
    await expect(page.getByRole('link', { name: 'har-test', exact: true })).toBeVisible({ timeout: 15000 });

    const expandButton = page.getByRole('button', { name: 'Show requests' });
    await expandButton.click();
    await expect(page.getByText('GET')).toBeVisible({ timeout: 10000 });

    const downloadPromise = page.waitForEvent('download');
    await page.getByRole('button', { name: 'Export as HAR' }).click();
    const download = await downloadPromise;

    expect(download.suggestedFilename()).toBe('har-test-history.har');

    const filePath = await download.path();
    expect(filePath).toBeTruthy();

    const fs = await import('fs');
    const content = fs.readFileSync(filePath!, 'utf-8');
    const har = JSON.parse(content);

    expect(har.log).toBeDefined();
    expect(har.log.version).toBe('1.2');
    expect(har.log.creator.name).toBe('Secutils.dev');
    expect(har.log.entries).toHaveLength(2);

    for (const entry of har.log.entries) {
      expect(entry.startedDateTime).toBeDefined();
      expect(entry.request.method).toBeDefined();
      expect(entry.request.url).toContain('/har-test');
      expect(entry.time).toBeGreaterThanOrEqual(0);
      expect(entry.timings).toBeDefined();
    }

    const methods = har.log.entries.map((e: { request: { method: string } }) => e.request.method);
    expect(methods).toContain('GET');
    expect(methods).toContain('POST');
  });
});
