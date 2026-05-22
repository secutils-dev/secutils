import { expect, test } from '@playwright/test';

import { assertSeoBasics, assertSkillMd, getTool } from './_helpers';

const tool = getTool('forecast');

test.describe(`${tool.name} (${tool.path})`, () => {
  test('SEO head block matches the AGENTS.md SEO budget', async ({ page }) => {
    const response = await page.goto(tool.path);
    expect(response?.ok()).toBeTruthy();
    await assertSeoBasics(page, tool);
  });

  test('skill .md is reachable with required frontmatter', async ({ request }) => {
    await assertSkillMd(request, tool);
  });

  test('?example=monthly-sales deep-link populates data + presets', async ({ page }) => {
    await page.goto(`${tool.path}?example=monthly-sales`);

    // The deep-link populates the textarea verbatim and applies the preset so
    // the controls reflect the curated configuration (linear fit, forecast 3,
    // no smoothing). Wait for the WASM bundle to load and the chart to paint.
    const input = page.locator('#dataInput');
    await expect(input).toHaveValue(/42000/);
    await expect(input).toHaveValue(/79000/);

    await expect(page.locator('#fitSelect')).toHaveValue('linear');
    await expect(page.locator('#smoothSelect')).toHaveValue('none');
    await expect(page.locator('#forecastHorizon')).toHaveValue('3');
    await expect(page.locator('#anomalySelect')).toHaveValue('none');

    // The chart SVG becomes visible (replaces the empty-state placeholder)
    // and the Results panel surfaces the Fit + Forecast cards.
    await expect(page.locator('#chartSvg')).toBeVisible({ timeout: 15000 });
    await expect(page.locator('#chartEmpty')).toBeHidden();

    const outputs = page.locator('#outputGrid');
    await expect(outputs).toContainText(/Fit/i, { timeout: 15000 });
    await expect(outputs).toContainText(/R²|R\^?2|R&sup2;|0\.99|0\.98/, { timeout: 15000 });
    await expect(outputs).toContainText(/Forecast/i);
  });

  test('selecting an example from the dropdown rewrites textarea and controls', async ({ page }) => {
    await page.goto(tool.path);

    await page.locator('#examplesDetails summary').click();
    await page.locator('[data-example-id="user-growth"]').click();

    await expect(page.locator('#dataInput')).toHaveValue(/1000[\s\S]+7500/);
    await expect(page.locator('#fitSelect')).toHaveValue('exponential');
    await expect(page.locator('#forecastHorizon')).toHaveValue('6');

    // doublingTime is the headline readout for the exponential preset.
    await expect(page.locator('#outputGrid')).toContainText(/Doubling time|doubling/i, {
      timeout: 15000,
    });
  });

  test('Share button copies a fragment URL that round-trips the data', async ({ page, context, browserName }) => {
    // The clipboard permission incantation differs across engines; only run
    // the round-trip half of this test in Chromium, where granting
    // `clipboard-read` is supported. The Share *button* itself is exercised
    // on every browser via the URL-fragment assertion below.
    await context.grantPermissions(['clipboard-read', 'clipboard-write']).catch(() => {});

    await page.goto(`${tool.path}?example=monthly-sales`);
    await expect(page.locator('#chartSvg')).toBeVisible({ timeout: 15000 });

    await page.locator('#shareBtn').click();
    await expect(page).toHaveURL(/#[A-Za-z0-9_-]+/, { timeout: 5000 });

    if (browserName !== 'chromium') return;

    const clipboard = await page.evaluate(() => navigator.clipboard.readText());
    expect(clipboard).toMatch(new RegExp(`${tool.path.replace(/[.*+?^${}()|[\\]\\\\]/g, '\\\\$&')}#[A-Za-z0-9_-]+`));

    // Open the copied URL in a fresh page and confirm the data hydrates
    // (round-trip through deflate-raw + base64url).
    const fresh = await context.newPage();
    await fresh.goto(clipboard);
    await expect(fresh.locator('#dataInput')).toHaveValue(/42000/);
    await fresh.close();
  });

  test('auto-best fit picks the linear family on a clean linear series', async ({ page }) => {
    await page.goto(tool.path);
    await page.locator('#dataInput').fill('1\n2\n3\n4\n5\n6\n7\n8\n9\n10');
    await page.locator('#fitSelect').selectOption('auto');

    // Allow time for the WASM bundle to lazy-load and the auto-fit search
    // to enumerate every candidate family.
    await expect(page.locator('#outputGrid')).toContainText(/linear/i, { timeout: 15000 });
    // Both R² and the Adjusted R² readouts should appear on the Fit card.
    await expect(page.locator('#outputGrid')).toContainText(/Adj\.\s*R/i, { timeout: 15000 });
  });

  test('smoothing-window slider is disabled when Smoothing = None', async ({ page }) => {
    await page.goto(tool.path);
    const smoothWindow = page.locator('#smoothWindow');
    await expect(smoothWindow).toBeDisabled();
    await page.locator('#smoothSelect').selectOption('sma');
    await expect(smoothWindow).toBeEnabled();
    await page.locator('#smoothSelect').selectOption('none');
    await expect(smoothWindow).toBeDisabled();
  });

  test('z-score threshold slider is disabled unless Anomalies = Residual z-score', async ({ page }) => {
    await page.goto(tool.path);
    const zSlider = page.locator('#zThreshold');
    await expect(zSlider).toBeDisabled();
    await page.locator('#anomalySelect').selectOption('peaks');
    await expect(zSlider).toBeDisabled();
    await page.locator('#anomalySelect').selectOption('zscore');
    await expect(zSlider).toBeEnabled();
    await page.locator('#anomalySelect').selectOption('none');
    await expect(zSlider).toBeDisabled();
  });

  test('logarithmic fit on auto-indexed data shifts x and reports it', async ({ page }) => {
    // The default index starts at x = 0; logarithmic regression requires
    // x > 0, so the tool shifts x by +1 transparently and labels the fit
    // accordingly. Without the shift this would surface a "x[0] = 0" error.
    await page.goto(tool.path);
    await page.locator('#dataInput').fill('1\n3\n5\n6\n7\n7.5\n8\n8.3\n8.5\n8.7');
    await page.locator('#fitSelect').selectOption('logarithmic');
    const outputs = page.locator('#outputGrid');
    await expect(outputs).toContainText(/logarithmic/i, { timeout: 15000 });
    await expect(outputs).toContainText(/x shifted by/i);
  });

  test('Walk-forward backtest renders the leaderboard + conformal band caption', async ({ page }) => {
    // Hand-pick a deterministic series long enough for the backtest (n >= 12).
    // The exact leaderboard ordering is sensitive to PRNG state, so the
    // assertions only check that the structural pieces show up, not which
    // family wins.
    await page.goto(tool.path);
    const data = Array.from({ length: 30 }, (_, i) => (10 + i * 0.5 + Math.sin(i / 3) * 2).toFixed(2)).join('\n');
    await page.locator('#dataInput').fill(data);
    await page.locator('#forecastHorizon').focus();
    await page.locator('#forecastHorizon').fill('6');
    await page.locator('#forecastHorizon').dispatchEvent('input');
    await page.locator('#backtestToggle').check();

    // The leaderboard cells are concatenated without whitespace in
    // `textContent`, so word-boundary anchors fail. Scope the assertions
    // to the actual <table> instead.
    const outputs = page.locator('#outputGrid');
    await expect(outputs).toContainText(/Backtest leaderboard/i, { timeout: 15000 });
    const leaderboardTable = page.locator('#outputGrid table.bt-table');
    await expect(leaderboardTable).toBeVisible();
    // The naive baseline row is always present so users can compare.
    await expect(leaderboardTable.locator('tr.naive')).toBeVisible();
    await expect(leaderboardTable.locator('tr.winner')).toBeVisible();
    // Leaderboard headers cover MAE / RMSE / Coverage.
    await expect(leaderboardTable.locator('th')).toContainText(['Model', 'MAE', 'RMSE', 'Coverage']);
    // Conformal caption replaces the legacy ±2σ caveat on the Forecast card.
    await expect(outputs).toContainText(/Conformal band/i);
    await expect(outputs).toContainText(/calibrated against/i);
  });

  test('Backtest hold-out + coverage sliders disable when toggle is off', async ({ page }) => {
    await page.goto(tool.path);
    const holdout = page.locator('#backtestHoldout');
    const coverage = page.locator('#backtestCoverage');
    await expect(holdout).toBeDisabled();
    await expect(coverage).toBeDisabled();
    await page.locator('#backtestToggle').check();
    await expect(holdout).toBeEnabled();
    await expect(coverage).toBeEnabled();
    await page.locator('#backtestToggle').uncheck();
    await expect(holdout).toBeDisabled();
    await expect(coverage).toBeDisabled();
  });

  test('Backtest is skipped with a warning on tiny series (n < 12)', async ({ page }) => {
    await page.goto(tool.path);
    await page.locator('#dataInput').fill('1\n2\n3\n4\n5\n6\n7\n8');
    await page.locator('#backtestToggle').check();
    const outputs = page.locator('#outputGrid');
    await expect(outputs).toContainText(/Backtest skipped/i, { timeout: 15000 });
    // The leaderboard card must NOT render in this case.
    await expect(outputs).not.toContainText(/Backtest leaderboard/i);
  });

  test('?example=backtest-trap deep-link flags naive-beats-all', async ({ page }) => {
    await page.goto(`${tool.path}?example=backtest-trap`);
    // Random walk preset enables backtest with K = 25% of 80 = 20 folds.
    await expect(page.locator('#backtestToggle')).toBeChecked();
    const outputs = page.locator('#outputGrid');
    await expect(outputs).toContainText(/Backtest leaderboard/i, { timeout: 15000 });
    // The signature banner: "Naive last-value beats every regression."
    await expect(outputs).toContainText(/Naive last-value beats every regression/i);
  });
});
