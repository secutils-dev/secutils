// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';

import { transformDevToolsRecording } from './devtools_recording_transformer';

const RETURN_HINT = [
  '',
  '  // TODO: Add a return statement with the content you want to track.',
  '  // Examples:',
  '  //   return await page.title();',
  '  //   return await page.content();',
  "  //   return await page.locator('.result').textContent();",
].join('\n');

function wrap(body: string) {
  return `export async function execute(page) {\n${body}\n${RETURN_HINT}\n}`;
}

describe('transformDevToolsRecording', () => {
  it('transforms a simple navigate + click recording', () => {
    const input = JSON.stringify({
      title: 'Test recording',
      steps: [
        {
          type: 'setViewport',
          width: 1280,
          height: 720,
          deviceScaleFactor: 1,
          isMobile: false,
          hasTouch: false,
          isLandscape: false,
        },
        { type: 'navigate', url: 'https://example.com' },
        { type: 'click', selectors: [['aria/Get started'], ['#get-started']], offsetX: 50, offsetY: 12 },
      ],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(
      wrap(["  await page.goto('https://example.com');", "  await page.getByLabel('Get started').click();"].join('\n')),
    );
  });

  it('transforms change steps to fill()', () => {
    const input = JSON.stringify({
      steps: [
        { type: 'navigate', url: 'https://example.com/form' },
        { type: 'change', selectors: [['aria/Email'], ['#email']], value: 'test@example.com' },
        { type: 'change', selectors: [['#password']], value: 'secret123' },
      ],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(
      wrap(
        [
          "  await page.goto('https://example.com/form');",
          "  await page.getByLabel('Email').fill('test@example.com');",
          "  await page.locator('#password').fill('secret123');",
        ].join('\n'),
      ),
    );
  });

  it('transforms doubleClick steps', () => {
    const input = JSON.stringify({
      steps: [{ type: 'doubleClick', selectors: [['#word']], offsetX: 10, offsetY: 5 }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.locator('#word').dblclick();"));
  });

  it('transforms hover steps', () => {
    const input = JSON.stringify({
      steps: [{ type: 'hover', selectors: [['aria/Menu'], ['.nav-menu']] }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.getByLabel('Menu').hover();"));
  });

  it('transforms keyDown steps', () => {
    const input = JSON.stringify({
      steps: [{ type: 'keyDown', key: 'Enter' }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.keyboard.press('Enter');"));
  });

  it('skips keyUp steps', () => {
    const input = JSON.stringify({
      steps: [
        { type: 'keyDown', key: 'Enter' },
        { type: 'keyUp', key: 'Enter' },
      ],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.keyboard.press('Enter');"));
  });

  it('transforms page-level scroll steps', () => {
    const input = JSON.stringify({
      steps: [{ type: 'scroll', x: 0, y: 500 }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap('  await page.evaluate(() => window.scrollTo(0, 500));'));
  });

  it('transforms element-level scroll steps', () => {
    const input = JSON.stringify({
      steps: [{ type: 'scroll', selectors: [['.scrollable']], x: 0, y: 200 }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.locator('.scrollable').evaluate((el) => el.scrollTo(0, 200));"));
  });

  it('transforms waitForElement steps', () => {
    const input = JSON.stringify({
      steps: [{ type: 'waitForElement', selectors: [['#loading']], visible: false }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.locator('#loading').waitFor({ state: 'hidden' });"));
  });

  it('transforms waitForElement with visible=true', () => {
    const input = JSON.stringify({
      steps: [{ type: 'waitForElement', selectors: [['#content']] }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.locator('#content').waitFor({ state: 'visible' });"));
  });

  it('transforms waitForExpression steps', () => {
    const input = JSON.stringify({
      steps: [{ type: 'waitForExpression', expression: 'document.readyState === "complete"' }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap('  await page.waitForFunction(\'document.readyState === "complete"\');'));
  });

  it('handles right-click (secondary button)', () => {
    const input = JSON.stringify({
      steps: [{ type: 'click', selectors: [['#ctx-menu']], button: 'secondary', offsetX: 10, offsetY: 10 }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.locator('#ctx-menu').click({ button: 'right' });"));
  });

  it('handles middle-click (auxiliary button)', () => {
    const input = JSON.stringify({
      steps: [{ type: 'click', selectors: [['a.link']], button: 'auxiliary', offsetX: 10, offsetY: 10 }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.locator('a.link').click({ button: 'middle' });"));
  });

  it('skips setViewport, emulateNetworkConditions, close, and customStep', () => {
    const input = JSON.stringify({
      steps: [
        {
          type: 'setViewport',
          width: 1280,
          height: 720,
          deviceScaleFactor: 1,
          isMobile: false,
          hasTouch: false,
          isLandscape: false,
        },
        { type: 'emulateNetworkConditions', download: 1000, upload: 1000, latency: 100 },
        { type: 'navigate', url: 'https://example.com' },
        { type: 'close' },
        { type: 'customStep', name: 'myStep', parameters: {} },
      ],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.goto('https://example.com');"));
  });

  it('prefers aria selectors over CSS', () => {
    const input = JSON.stringify({
      steps: [{ type: 'click', selectors: [['#submit'], ['aria/Submit[role="button"]']], offsetX: 10, offsetY: 10 }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.getByRole('button', { name: 'Submit' }).click();"));
  });

  it('uses text selectors when no aria or CSS available', () => {
    const input = JSON.stringify({
      steps: [{ type: 'click', selectors: [['text/Click me']], offsetX: 10, offsetY: 10 }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.getByText('Click me').click();"));
  });

  it('throws on invalid JSON', () => {
    expect(() => transformDevToolsRecording('not json')).toThrow('Invalid JSON');
  });

  it('throws on missing steps array', () => {
    expect(() => transformDevToolsRecording('{"title":"test"}')).toThrow('missing "steps" array');
  });

  it('adds comment for unknown step types', () => {
    const input = JSON.stringify({
      steps: [{ type: 'navigate', url: 'https://example.com' }, { type: 'unknownNewType' }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toContain('// Unsupported step type: unknownNewType');
  });

  it('handles aria selector without role', () => {
    const input = JSON.stringify({
      steps: [{ type: 'click', selectors: [['aria/Username']], offsetX: 10, offsetY: 10 }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.getByLabel('Username').click();"));
  });

  it('handles aria selector with role but no name', () => {
    const input = JSON.stringify({
      steps: [{ type: 'click', selectors: [['aria/[role="button"]']], offsetX: 10, offsetY: 10 }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.getByRole('button').click();"));
  });

  it('handles empty recording steps', () => {
    const input = JSON.stringify({ steps: [] });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap(''));
  });

  it('handles nested shadow DOM selectors (uses last element)', () => {
    const input = JSON.stringify({
      steps: [{ type: 'click', selectors: [['.host', '.shadow-child', '#target']], offsetX: 10, offsetY: 10 }],
    });

    const result = transformDevToolsRecording(input);
    expect(result).toBe(wrap("  await page.locator('#target').click();"));
  });
});
