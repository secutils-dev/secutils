import { EuiDataGridObject } from '@elastic/eui-test-helpers';
import { expect, test } from '@playwright/test';

import { ensureUserAndLogin } from '../helpers';

// Wide enough that the resize is unmistakable even if the grid re-flows neighbouring columns.
const RESIZE_DELTA = 120;

// The requests grid auto-refreshes every 3s; wait past two intervals when asserting that a
// refresh did *not* happen.
const AUTO_REFRESH_INTERVAL_MS = 3000;

test.describe('Responder Requests Grid', () => {
  test.beforeEach(async ({ request, page }) => {
    await ensureUserAndLogin(request, page);
  });

  test('retains column widths across auto-refresh and manual refresh', async ({ page }) => {
    const stateResponse = await page.request.get('/api/ui/state');
    expect(stateResponse.ok()).toBeTruthy();
    const state = await stateResponse.json();

    const createResponse = await page.request.post('/api/webhooks/responders', {
      data: {
        name: 'grid-state-test',
        location: { pathType: '=', path: '/grid-state-test', subdomainPrefix: null },
        method: 'ANY',
        enabled: true,
        settings: {
          requestsToTrack: 10,
          statusCode: 200,
          body: 'hello-grid-state',
          headers: [['content-type', 'text/plain']],
        },
      },
    });
    expect(createResponse.ok()).toBeTruthy();
    const responder = await createResponse.json();

    const webhookUrl = `http://${state.user.handle}.webhooks.localhost:7171/grid-state-test`;
    const sendRequest = async () => {
      const response = await page.request.fetch(webhookUrl, { method: 'GET' });
      expect(response.ok()).toBeTruthy();
    };
    await sendRequest();

    await page.goto(`/ws/webhooks__responders?q=${responder.id}`);
    await expect(page.getByRole('link', { name: 'grid-state-test', exact: true })).toBeVisible({ timeout: 15000 });
    await page.getByRole('button', { name: 'Show requests' }).click();

    // One cell per recorded request, so the count tracks how many requests the grid shows.
    const grid = new EuiDataGridObject(page, 'responder-requests-grid');
    const requestRows = grid.cells('timestamp');
    await expect(requestRows).toHaveCount(1, { timeout: 15000 });

    // Raw locators: the component object covers rows and cells, but EUI exposes neither the
    // header cell nor its resize handle via a role.
    const urlHeader = page.locator('[data-test-subj="dataGridHeaderCell-url"]');
    const resizer = urlHeader.locator('[data-test-subj="dataGridColumnResizer"]');
    await expect(resizer).toBeAttached();

    const headerBox = async () => {
      const box = await urlHeader.boundingBox();
      expect(box).not.toBeNull();
      return box!;
    };

    // The resize handle is a zero-width hit zone whose only painted area is a `::after`
    // pseudo-element straddling the column border, so it cannot be hovered as an element -
    // drive the mouse over the border itself.
    const initialBox = await headerBox();
    const borderX = initialBox.x + initialBox.width;
    const centerY = initialBox.y + initialBox.height / 2;
    await page.mouse.move(borderX, centerY);
    await page.mouse.down();
    await page.mouse.move(borderX + RESIZE_DELTA, centerY, { steps: 5 });
    await page.mouse.up();

    const resizedWidth = (await headerBox()).width;
    expect(resizedWidth).toBeGreaterThan(initialBox.width + RESIZE_DELTA / 2);

    // A newly recorded request proves an auto-refresh actually landed, rather than the test
    // merely sleeping past the poll interval.
    await sendRequest();
    await expect(requestRows).toHaveCount(2, { timeout: 15000 });
    expect((await headerBox()).width).toBeCloseTo(resizedWidth, 0);

    // With auto-refresh off the grid can only pick up the next request via "Update", which
    // used to swap the whole grid for a loading state and reset the widths with it.
    await page.getByRole('switch', { name: 'Auto-refresh' }).click();
    await sendRequest();
    await page.waitForTimeout(AUTO_REFRESH_INTERVAL_MS * 2);
    await expect(requestRows).toHaveCount(2);

    // Exact match: the responders grid has a "Last updated" column sort button.
    await page.getByRole('button', { name: 'Update', exact: true }).click();
    await expect(requestRows).toHaveCount(3, { timeout: 15000 });
    expect((await headerBox()).width).toBeCloseTo(resizedWidth, 0);
  });
});
