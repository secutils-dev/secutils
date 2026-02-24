import { join } from 'path';

import { expect, test } from '@playwright/test';

import {
  dismissAllToasts,
  DOCS_IMG_DIR,
  EMAIL,
  ensureUserAndLogin,
  fixTrackerResourceRevisions,
  goto,
  highlightOff,
  highlightOn,
  PASSWORD,
} from '../helpers';

const IMG_DIR = join(DOCS_IMG_DIR, 'web_scraping');

const RESOURCE_TABLE_COLUMNS = [
  { id: 'source', label: 'Source', sortable: true },
  { id: 'diff', label: 'Diff', sortable: true },
  { id: 'type', label: 'Type', sortable: true },
  { id: 'size', label: 'Size', sortable: true },
];

function mockResourceRevision(rows: Array<Record<string, unknown>>, id = '00000000-0000-7000-8000-000000000001') {
  return {
    id,
    trackerId: '00000000-0000-7000-8000-000000000000',
    data: {
      original: {
        '@secutils.data.view': 'table',
        columns: RESOURCE_TABLE_COLUMNS,
        rows,
        source: {
          scripts: rows
            .filter((r) => r.type === 'Script')
            .map((r) => ({ type: 'script', url: r.source, content: { size: Number(r.size), data: {} } })),
          styles: rows
            .filter((r) => r.type === 'Stylesheet')
            .map((r) => ({ type: 'stylesheet', url: r.source, content: { size: Number(r.size), data: {} } })),
        },
      },
    },
    createdAt: 1740000000,
  };
}

test.describe('Web scraping guide screenshots', () => {
  test.beforeEach(async ({ page, request }) => {
    await ensureUserAndLogin(request, page, { email: EMAIL, password: PASSWORD });
  });

  test('Create a page tracker', async ({ page }) => {
    const extractorScript = [
      'export async function execute(page) {',
      '  // Navigate to the Hacker News homepage.',
      "  await page.goto('https://news.ycombinator.com/');",
      '',
      '  // Get the link to the top post.',
      "  const titleLink = page.locator('css=.titleline a').first();",
      '',
      '  // Return the title and link of the top post formatted as markdown.',
      "  return `[${(await titleLink.textContent()).trim()}](${await titleLink.getAttribute('href')})`;",
      '};',
    ].join('\n');

    // Step 1: Navigate to page trackers and show the empty state.
    await goto(page, '/ws/web_scraping__page');
    const trackPageButton = page.getByRole('button', { name: 'Track page' });
    await expect(trackPageButton).toBeVisible({ timeout: 15000 });
    await highlightOn(trackPageButton);
    await page.screenshot({ path: join(IMG_DIR, 'create_step1_empty.png') });

    // Create the tracker via API (Monaco editor cannot be reliably filled via Playwright).
    const createResponse = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'Hacker News Top Post',
        config: { revisions: 3 },
        target: { extractor: extractorScript },
      },
    });
    expect(createResponse.ok()).toBeTruthy();

    // Step 2: Reload to see the tracker, open Edit, and screenshot the form with the script.
    await goto(page, '/ws/web_scraping__page');
    const trackerRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Hacker News Top Post' }) });
    await expect(trackerRow).toBeVisible({ timeout: 15000 });

    await trackerRow.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    await flyout.getByText('Content extractor').first().scrollIntoViewIfNeeded();
    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveButton);
    await page.screenshot({ path: join(IMG_DIR, 'create_step2_form.png') });

    await flyout.getByRole('button', { name: 'Close' }).click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });
    await highlightOn(trackerRow);
    await page.screenshot({ path: join(IMG_DIR, 'create_step3_created.png') });
    await highlightOff(trackerRow);

    // Step 4: Show the tracker in the grid with an expanded empty state and Update button.
    await trackerRow.getByRole('button', { name: 'Show history' }).click();
    const updateButton = page.getByRole('button', { name: 'Update', exact: true });
    await expect(updateButton).toBeVisible({ timeout: 10000 });
    await highlightOn(updateButton);
    await page.screenshot({ path: join(IMG_DIR, 'create_step4_update.png') });

    // Step 5: Click Update to fetch content, replacing the dynamic link and timestamp with fixed values.
    const FIXED_CONTENT = '[All-in-one security toolbox for engineers and researchers](https://secutils.dev)';
    const FIXED_REVISION_TIMESTAMP = 1735689600; // Jan 1, 2025 00:00:00 UTC
    await page.route('**/api/utils/web_scraping/page/*/history', async (route) => {
      const response = await route.fetch();
      const json = await response.json();
      for (const rev of json) {
        rev.createdAt = FIXED_REVISION_TIMESTAMP;
        if (typeof rev.data?.original === 'string') {
          rev.data.original = FIXED_CONTENT;
        }
      }
      await route.fulfill({ response, json });
    });

    await updateButton.click();
    await expect(page.getByText('All-in-one security toolbox')).toBeVisible({ timeout: 60000 });
    await page.screenshot({ path: join(IMG_DIR, 'create_step5_result.png') });
  });

  test('Detect changes with a page tracker', async ({ page }) => {
    const extractorScript = [
      'export async function execute(page) {',
      '  // Navigate to the Berlin world clock page.',
      "  await page.goto('https://www.timeanddate.com/worldclock/germany/berlin');",
      '',
      '  // Wait for the time element to be visible and get its value.',
      "  const time = await page.locator('css=#qlook #ct').textContent();",
      '',
      '  // Return the time formatted as markdown with a link to the world clock page.',
      '  return `Berlin time is [**${time}**](https://www.timeanddate.com/worldclock/germany/berlin)`;',
      '};',
    ].join('\n');

    // Step 1: Navigate to page trackers and show the empty state.
    await goto(page, '/ws/web_scraping__page');
    const trackPageButton = page.getByRole('button', { name: 'Track page' });
    await expect(trackPageButton).toBeVisible({ timeout: 15000 });
    await highlightOn(trackPageButton);
    await page.screenshot({ path: join(IMG_DIR, 'detect_step1_empty.png') });

    // Create the tracker via API with hourly schedule and notifications.
    const createResponse = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'World Clock',
        config: { revisions: 3, job: { schedule: '@hourly' } },
        target: { extractor: extractorScript },
        notifications: true,
      },
    });
    expect(createResponse.ok()).toBeTruthy();

    // Step 2: Reload to see the tracker, open Edit, and screenshot the form with the script.
    await goto(page, '/ws/web_scraping__page');
    const trackerRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'World Clock' }) });
    await expect(trackerRow).toBeVisible({ timeout: 15000 });

    await trackerRow.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    await flyout.getByText('Content extractor').first().scrollIntoViewIfNeeded();
    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveButton);
    await page.screenshot({ path: join(IMG_DIR, 'detect_step2_form.png') });

    await flyout.getByRole('button', { name: 'Close' }).click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    // Step 3: Show the tracker in the grid (bell and timer icons indicate scheduled checks with notifications).
    await highlightOn(trackerRow);
    await page.screenshot({ path: join(IMG_DIR, 'detect_step3_created.png') });
  });

  test('Track web page resources', async ({ page }) => {
    test.setTimeout(120000);

    const extractorScript = [
      'export async function execute(page, { previousContent }) {',
      '  // Load built-in utilities for tracking resources.',
      '  const { resources: utils } = await import(`data:text/javascript,${encodeURIComponent(',
      "    await (await fetch('https://secutils.dev/retrack/utilities.js')).text()",
      '  )}`);',
      '',
      '  // Start tracking resources.',
      '  utils.startTracking(page);',
      '',
      '  // Navigate to the target page.',
      "  await page.goto('https://news.ycombinator.com');",
      '  await page.waitForTimeout(1000);',
      '',
      '  // Stop tracking and return resources.',
      '  const resources = await utils.stopTracking(page);',
      '',
      '  // Format resources as a table,',
      '  // showing diff status if previous content is available.',
      '  return utils.formatAsTable(',
      '    previousContent',
      '      ? utils.setDiffStatus(previousContent.original.source, resources)',
      '      : resources',
      '  );',
      '};',
    ].join('\n');

    // Step 1: Navigate to page trackers and show the empty state.
    await goto(page, '/ws/web_scraping__page');
    const trackPageButton = page.getByRole('button', { name: 'Track page' });
    await expect(trackPageButton).toBeVisible({ timeout: 15000 });
    await highlightOn(trackPageButton);
    await page.screenshot({ path: join(IMG_DIR, 'resources_step1_empty.png') });

    // Create the tracker via API.
    const createResponse = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'Hacker News (resources)',
        config: { revisions: 3 },
        target: { extractor: extractorScript },
      },
    });
    expect(createResponse.ok()).toBeTruthy();

    // Step 2: Reload to see the tracker, open Edit, and screenshot the form with the script.
    await goto(page, '/ws/web_scraping__page');
    const trackerRow = page
      .getByRole('row')
      .filter({ has: page.getByRole('cell', { name: 'Hacker News (resources)' }) });
    await expect(trackerRow).toBeVisible({ timeout: 15000 });

    await trackerRow.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    await flyout.getByText('Content extractor').first().scrollIntoViewIfNeeded();
    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveButton);
    await page.screenshot({ path: join(IMG_DIR, 'resources_step2_form.png') });

    await flyout.getByRole('button', { name: 'Close' }).click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    // Step 3: Show the tracker in the grid with an expanded empty state.
    await trackerRow.getByRole('button', { name: 'Show history' }).click();
    const updateButton = page.getByRole('button', { name: 'Update', exact: true });
    await expect(updateButton).toBeVisible({ timeout: 10000 });
    await highlightOn(updateButton);
    await page.screenshot({ path: join(IMG_DIR, 'resources_step3_created.png') });

    // Step 4: Click Update to fetch resources, with route interception for stability.
    await fixTrackerResourceRevisions(page);
    await updateButton.click();
    await expect(page.getByText('hn.js')).toBeVisible({ timeout: 60000 });
    await page.screenshot({ path: join(IMG_DIR, 'resources_step4_result.png') });
  });

  test('Filter web page resources', async ({ page }) => {
    const extractorScript = [
      'export async function execute(page, { previousContent }) {',
      '  // Load built-in utilities for tracking resources.',
      '  const { resources: utils } = await import(`data:text/javascript,${encodeURIComponent(',
      "    await (await fetch('https://secutils.dev/retrack/utilities.js')).text()",
      '  )}`);',
      '',
      '  // Start tracking resources.',
      '  utils.startTracking(page);',
      '',
      '  // Navigate to the target page.',
      "  await page.goto('https://github.com');",
      '  await page.waitForTimeout(1000);',
      '',
      '  // Stop tracking and return resources.',
      '  const resources = await utils.stopTracking(page);',
      '',
      '  // Format resources as a table,',
      '  // showing diff status if previous content is available.',
      '  return utils.formatAsTable(',
      '    previousContent',
      '      ? utils.setDiffStatus(previousContent.original.source, resources)',
      '      : resources',
      '  );',
      '};',
    ].join('\n');

    // Step 1: Navigate to page trackers and show the empty state.
    await goto(page, '/ws/web_scraping__page');
    const trackPageButton = page.getByRole('button', { name: 'Track page' });
    await expect(trackPageButton).toBeVisible({ timeout: 15000 });
    await highlightOn(trackPageButton);
    await page.screenshot({ path: join(IMG_DIR, 'filter_step1_empty.png') });

    // Create the tracker via API.
    const createResponse = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'GitHub',
        config: { revisions: 3 },
        target: { extractor: extractorScript },
      },
    });
    expect(createResponse.ok()).toBeTruthy();

    // Step 2: Reload to see the tracker, open Edit, and screenshot the form with the script.
    await goto(page, '/ws/web_scraping__page');
    const trackerRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'GitHub' }) });
    await expect(trackerRow).toBeVisible({ timeout: 15000 });

    await trackerRow.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    await flyout.getByText('Content extractor').first().scrollIntoViewIfNeeded();
    const saveButton = flyout.getByRole('button', { name: 'Save' });
    await highlightOn(saveButton);
    await page.screenshot({ path: join(IMG_DIR, 'filter_step2_form.png') });

    await flyout.getByRole('button', { name: 'Close' }).click();
    await expect(flyout).not.toBeVisible({ timeout: 10000 });

    // Step 3: Show tracker in grid with expanded empty state and Update button.
    await trackerRow.getByRole('button', { name: 'Show history' }).click();
    const updateButton = page.getByRole('button', { name: 'Update', exact: true });
    await expect(updateButton).toBeVisible({ timeout: 10000 });
    await highlightOn(updateButton);
    await page.screenshot({ path: join(IMG_DIR, 'filter_step3_created.png') });

    // Step 4: Click Update with mocked response. Unlike HN which has stable resources,
    // GitHub rebuilds asset bundles with new hash-based filenames on every deploy, making
    // real fetches produce different screenshots each time.
    await page.route('**/api/utils/web_scraping/page/*/history', async (route) => {
      const body = route.request().postDataJSON();
      if (!body?.refresh) {
        return route.continue();
      }
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([
          mockResourceRevision([
            {
              source: 'https://github.githubassets.com/assets/vendors-node_modules_github_mini-throttle.js',
              type: 'Script',
              size: '4200',
            },
            {
              source: 'https://github.githubassets.com/assets/vendors-node_modules_github_catalyst.js',
              type: 'Script',
              size: '8100',
            },
            {
              source: 'https://github.githubassets.com/assets/vendors-node_modules_primer_behaviors.js',
              type: 'Script',
              size: '12300',
            },
            {
              source: 'https://github.githubassets.com/assets/vendors-node_modules_dompurify.js',
              type: 'Script',
              size: '6800',
            },
            {
              source: 'https://github.githubassets.com/assets/vendors-node_modules_lit-html.js',
              type: 'Script',
              size: '9500',
            },
            {
              source: 'https://github.githubassets.com/assets/vendors-node_modules_github_selector-observer.js',
              type: 'Script',
              size: '3700',
            },
            {
              source: 'https://github.githubassets.com/assets/vendors-node_modules_github_relative-time-element.js',
              type: 'Script',
              size: '5200',
            },
            { source: 'https://github.githubassets.com/assets/environment.js', type: 'Script', size: '1200' },
            { source: 'https://github.githubassets.com/assets/behaviors.js', type: 'Script', size: '3200' },
            {
              source: 'https://github.githubassets.com/assets/primer-primitives.css',
              type: 'Stylesheet',
              size: '45200',
            },
            { source: 'https://github.githubassets.com/assets/primer-react.css', type: 'Stylesheet', size: '28400' },
            { source: 'https://github.githubassets.com/assets/global.css', type: 'Stylesheet', size: '18700' },
          ]),
        ]),
      });
    });
    await updateButton.click();
    await expect(page.getByText('github.githubassets.com').first()).toBeVisible({ timeout: 15000 });
    await page.screenshot({ path: join(IMG_DIR, 'filter_step4_result.png') });
  });

  // Responder URLs use the public_url (localhost:7171) which isn't reachable from the
  // Docker web scraper container, so this test uses fully mocked revision data.
  test('Detect changes in web page resources', async ({ page }) => {
    const JS_HEADER = 'Content-Type: application/javascript; charset=utf-8';
    const HTML_HEADER = 'Content-Type: text/html; charset=utf-8';

    const jsBody = (name: string, extra = '') =>
      `document.body.insertAdjacentHTML(\n  'beforeend',\n  'Source: ${name}${extra}<br>'\n);`;

    const extractorScript = [
      'export async function execute(page, { previousContent }) {',
      '  const { resources: utils } = await import(`data:text/javascript,${encodeURIComponent(',
      "    await (await fetch('https://secutils.dev/retrack/utilities.js')).text()",
      '  )}`);',
      '  utils.startTracking(page);',
      "  await page.goto('https://preview.webhooks.secutils.dev/track-me.html');",
      '  await page.waitForTimeout(1000);',
      '  const resources = await utils.stopTracking(page);',
      '  return utils.formatAsTable(',
      '    previousContent',
      '      ? utils.setDiffStatus(previousContent.original.source, resources)',
      '      : resources',
      '  );',
      '};',
    ].join('\n');

    // Helper to create a JS responder via the UI form and take a screenshot.
    async function createJsResponder(name: string, path: string, body: string, screenshotName: string) {
      const createButton = page.getByRole('button', { name: 'Create responder' });
      await expect(createButton).toBeVisible({ timeout: 15000 });
      await createButton.click();

      const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add responder' }) });
      await expect(flyout).toBeVisible();

      await flyout.getByLabel('Name').fill(name);
      await flyout.getByLabel('Path', { exact: true }).fill(path);

      await flyout.getByRole('button', { name: /Remove Content-Type/ }).click();
      const headersCombo = flyout.getByRole('combobox', { name: 'Headers' });
      await headersCombo.fill(JS_HEADER);
      await headersCombo.press('Enter');

      const bodyTextarea = flyout.getByLabel('Body');
      await bodyTextarea.fill(body);
      await bodyTextarea.evaluate((el) => (el.scrollTop = 0));

      const saveButton = flyout.getByRole('button', { name: 'Save' });
      await highlightOn(saveButton);
      await page.screenshot({ path: join(IMG_DIR, screenshotName) });

      await saveButton.click();
      await expect(flyout).not.toBeVisible({ timeout: 10000 });
      await dismissAllToasts(page);
    }

    // Step 1: Navigate to Webhooks → Responders and show the empty state.
    await goto(page, '/ws/webhooks__responders');
    const createButton = page.getByRole('button', { name: 'Create responder' });
    await expect(createButton).toBeVisible({ timeout: 15000 });
    await highlightOn(createButton);
    await page.screenshot({ path: join(IMG_DIR, 'detect_resources_step1_responders_empty.png') });

    // Steps 2–5: Create each JS responder via UI with a screenshot.
    await createJsResponder(
      'no-changes.js',
      '/no-changes.js',
      jsBody('no-changes.js'),
      'detect_resources_step2_no_changes_form.png',
    );
    await createJsResponder(
      'changed.js',
      '/changed.js',
      jsBody('changed.js', ', Changed: no'),
      'detect_resources_step3_changed_form.png',
    );
    await createJsResponder(
      'removed.js',
      '/removed.js',
      jsBody('removed.js'),
      'detect_resources_step4_removed_form.png',
    );
    await createJsResponder('added.js', '/added.js', jsBody('added.js'), 'detect_resources_step5_added_form.png');

    // Step 6: Create `track-me.html` responder via UI.
    await createButton.click();
    const addFlyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Add responder' }) });
    await expect(addFlyout).toBeVisible();

    await addFlyout.getByLabel('Name').fill('track-me.html');
    await addFlyout.getByLabel('Path', { exact: true }).fill('/track-me.html');

    await addFlyout.getByRole('button', { name: /Remove Content-Type/ }).click();
    const htmlHeadersCombo = addFlyout.getByRole('combobox', { name: 'Headers' });
    await htmlHeadersCombo.fill(HTML_HEADER);
    await htmlHeadersCombo.press('Enter');

    const htmlBody = [
      '<!DOCTYPE html>',
      '<html lang="en">',
      '<head>',
      '  <title>Evaluate resources tracker</title>',
      '  <script type="text/javascript" src="./no-changes.js" defer></script>',
      '  <script type="text/javascript" src="./changed.js" defer></script>',
      '  <script type="text/javascript" src="./removed.js" defer></script>',
      '</head>',
      '<body></body>',
      '</html>',
    ].join('\n');
    const htmlBodyTextarea = addFlyout.getByLabel('Body');
    await htmlBodyTextarea.fill(htmlBody);
    await htmlBodyTextarea.evaluate((el) => (el.scrollTop = 0));

    const htmlSaveButton = addFlyout.getByRole('button', { name: 'Save' });
    await highlightOn(htmlSaveButton);
    await page.screenshot({ path: join(IMG_DIR, 'detect_resources_step6_html_form.png') });

    await htmlSaveButton.click();
    await expect(addFlyout).not.toBeVisible({ timeout: 10000 });
    await dismissAllToasts(page);

    // Step 7: Show all 5 responders in the grid.
    await goto(page, '/ws/webhooks__responders');
    const noChangesRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'no-changes.js' }) });
    await expect(noChangesRow).toBeVisible({ timeout: 15000 });
    const trackMeRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'track-me.html' }) });
    await expect(trackMeRow).toBeVisible();
    await page.screenshot({ path: join(IMG_DIR, 'detect_resources_step7_responders_created.png') });

    // Step 8: Navigate to page trackers and show the empty state.
    await goto(page, '/ws/web_scraping__page');
    const trackPageButton = page.getByRole('button', { name: 'Track page' });
    await expect(trackPageButton).toBeVisible({ timeout: 15000 });
    await highlightOn(trackPageButton);
    await page.screenshot({ path: join(IMG_DIR, 'detect_resources_step8_trackers_empty.png') });

    // Create page tracker via API.
    const createTrackerRes = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'Demo',
        config: { revisions: 3 },
        target: { extractor: extractorScript },
      },
    });
    expect(createTrackerRes.ok()).toBeTruthy();

    // Step 9: Reload to see the tracker, open Edit, and screenshot the form with the script.
    await goto(page, '/ws/web_scraping__page');
    const trackerRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Demo' }) });
    await expect(trackerRow).toBeVisible({ timeout: 15000 });

    await trackerRow.getByRole('button', { name: 'Edit' }).click();
    const editFlyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(editFlyout).toBeVisible();

    await editFlyout.getByText('Content extractor').first().scrollIntoViewIfNeeded();
    const trackerSaveButton = editFlyout.getByRole('button', { name: 'Save' });
    await highlightOn(trackerSaveButton);
    await page.screenshot({ path: join(IMG_DIR, 'detect_resources_step9_tracker_form.png') });

    await editFlyout.getByRole('button', { name: 'Close' }).click();
    await expect(editFlyout).not.toBeVisible({ timeout: 10000 });

    // Step 10: Expand tracker row and show Update button.
    await trackerRow.getByRole('button', { name: 'Show history' }).click();
    const updateButton = page.getByRole('button', { name: 'Update', exact: true });
    await expect(updateButton).toBeVisible({ timeout: 10000 });
    await highlightOn(updateButton);
    await page.screenshot({ path: join(IMG_DIR, 'detect_resources_step10_tracker_created.png') });

    // Set up mocked route handler for stable screenshots.
    const BASE = 'https://preview.webhooks.secutils.dev';
    const initialRows = [
      { source: `${BASE}/no-changes.js`, type: 'Script', size: '81' },
      { source: `${BASE}/changed.js`, type: 'Script', size: '91' },
      { source: `${BASE}/removed.js`, type: 'Script', size: '78' },
    ];
    const diffRows = [
      { source: `${BASE}/no-changes.js`, diff: '-', type: 'Script', size: '81' },
      { source: `${BASE}/changed.js`, diff: { value: 'Changed', color: 'warning' }, type: 'Script', size: '92' },
      { source: `${BASE}/added.js`, diff: { value: 'Added', color: 'success' }, type: 'Script', size: '76' },
      { source: `${BASE}/removed.js`, diff: { value: 'Removed', color: 'danger' }, type: 'Script', size: '78' },
    ];

    let updateCount = 0;
    await page.route('**/api/utils/web_scraping/page/*/history', async (route) => {
      const body = route.request().postDataJSON();
      if (!body?.refresh) {
        return route.continue();
      }
      updateCount++;
      const rows = updateCount === 1 ? initialRows : diffRows;
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify([mockResourceRevision(rows, `00000000-0000-7000-8000-00000000000${updateCount}`)]),
      });
    });

    // Step 11: Click Update to fetch initial resources (3 scripts, no diff).
    await updateButton.click();
    await expect(page.getByText('no-changes.js')).toBeVisible({ timeout: 15000 });
    await page.screenshot({ path: join(IMG_DIR, 'detect_resources_step11_initial.png') });

    // Step 12: Click Update again to fetch updated resources showing diff statuses.
    await updateButton.click();
    await expect(page.getByText('Added', { exact: true })).toBeVisible({ timeout: 15000 });
    await page.screenshot({ path: join(IMG_DIR, 'detect_resources_step12_diff.png') });
  });

  test('Custom cron schedule', async ({ page }) => {
    // Mock the schedule parse API to return fixed upcoming check dates.
    await page.route('**/api/scheduler/parse_schedule', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          minInterval: 31536000,
          nextOccurrences: [
            1759936260, // Wed, 08 Oct 2025 15:11:00 GMT
            1791472260, // Thu, 08 Oct 2026 15:11:00 GMT
            1823008260, // Fri, 08 Oct 2027 15:11:00 GMT
            1854630660, // Sun, 08 Oct 2028 15:11:00 GMT
            1886166660, // Mon, 08 Oct 2029 15:11:00 GMT
          ],
        }),
      });
    });

    // Create a tracker with a custom cron schedule via API.
    const createResponse = await page.request.post('/api/utils/web_scraping/page', {
      data: {
        name: 'Custom Schedule Demo',
        config: { revisions: 3, job: { schedule: '0 11 15 8 10 ?' } },
        target: { extractor: 'export async function execute() { return "test"; }' },
        notifications: true,
      },
    });
    expect(createResponse.ok()).toBeTruthy();

    // Open the Edit flyout for the tracker.
    await goto(page, '/ws/web_scraping__page');
    const trackerRow = page.getByRole('row').filter({ has: page.getByRole('cell', { name: 'Custom Schedule Demo' }) });
    await expect(trackerRow).toBeVisible({ timeout: 15000 });

    await trackerRow.getByRole('button', { name: 'Edit' }).click();
    const flyout = page.getByRole('dialog').filter({ has: page.getByRole('heading', { name: 'Edit tracker' }) });
    await expect(flyout).toBeVisible();

    // Wait for the calendar icon to appear (schedule parse succeeded).
    const calendarButton = flyout.getByLabel('Show next occurrences');
    await expect(calendarButton).toBeVisible({ timeout: 15000 });

    // Scroll so the "Change tracking" section is visible.
    const changeTrackingHeading = flyout.getByRole('heading', { name: 'Change tracking', level: 3 });
    await changeTrackingHeading.scrollIntoViewIfNeeded();

    // Hover the calendar button to show the "Upcoming checks" tooltip.
    await calendarButton.hover();
    const tooltip = page.getByRole('tooltip');
    await expect(tooltip).toBeVisible({ timeout: 5000 });
    await highlightOn(tooltip);

    // Clip the screenshot to the "Change tracking" section plus the tooltip.
    const section = flyout
      .locator('.euiDescribedFormGroup')
      .filter({ has: page.locator('h3', { hasText: 'Change tracking' }) });
    const sectionBox = (await section.boundingBox())!;
    const tooltipBox = (await tooltip.boundingBox())!;
    const PAD = 10;
    const x = Math.min(sectionBox.x, tooltipBox.x) - PAD;
    const y = Math.min(sectionBox.y, tooltipBox.y) - PAD;
    const right = Math.max(sectionBox.x + sectionBox.width, tooltipBox.x + tooltipBox.width) + PAD;
    const bottom = Math.max(sectionBox.y + sectionBox.height, tooltipBox.y + tooltipBox.height) + PAD;

    await page.screenshot({
      path: join(IMG_DIR, 'custom_schedule.png'),
      clip: { x, y, width: right - x, height: bottom - y },
    });
  });
});
