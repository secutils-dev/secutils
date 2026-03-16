/**
 * Schema coverage test for the Chrome DevTools Recorder JSON transformer.
 *
 * Verifies that the transformer handles every known step type from the
 * Chrome DevTools Recorder / @puppeteer/replay schema. When new step
 * types are added to the schema, a new entry should be added to the
 * STEP_TYPE_SAMPLES map below. Failing to do so does not break the
 * transformer (unknown types produce a comment), but this test ensures
 * we explicitly decide how to handle each one.
 *
 * The canonical step types are defined in the StepType enum in @puppeteer/replay:
 *   https://github.com/puppeteer/replay/blob/a90a118766de8987a8a91d82b83681b4a657076d/src/Schema.ts#L32
 *
 * Run:
 *   cd components/secutils-webui
 *   npx vitest run src/pages/workspace/utils/web_scraping/devtools_recording_transformer_schema.test.ts
 */
// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';

import { transformDevToolsRecording } from './devtools_recording_transformer';

/**
 * Every known step type from the @puppeteer/replay StepType enum (v3.1.3),
 * with a minimal sample step payload and the expected Playwright output.
 *
 * `expectedContains`:
 *   - `null` means the step should be silently skipped (no output line)
 *   - a string means the output should contain that substring
 */
const STEP_TYPE_SAMPLES: Array<{
  type: string;
  step: Record<string, unknown>;
  expectedContains: string | null;
}> = [
  {
    type: 'navigate',
    step: { type: 'navigate', url: 'https://example.com' },
    expectedContains: "page.goto('https://example.com')",
  },
  {
    type: 'click',
    step: { type: 'click', selectors: [['#btn']], offsetX: 10, offsetY: 10 },
    expectedContains: '.click()',
  },
  {
    type: 'doubleClick',
    step: { type: 'doubleClick', selectors: [['#btn']], offsetX: 10, offsetY: 10 },
    expectedContains: '.dblclick()',
  },
  {
    type: 'hover',
    step: { type: 'hover', selectors: [['#link']] },
    expectedContains: '.hover()',
  },
  {
    type: 'change',
    step: { type: 'change', selectors: [['#input']], value: 'hello' },
    expectedContains: ".fill('hello')",
  },
  {
    type: 'keyDown',
    step: { type: 'keyDown', key: 'Tab' },
    expectedContains: "keyboard.press('Tab')",
  },
  {
    type: 'keyUp',
    step: { type: 'keyUp', key: 'Tab' },
    expectedContains: null,
  },
  {
    type: 'scroll (page)',
    step: { type: 'scroll', x: 0, y: 100 },
    expectedContains: 'window.scrollTo(0, 100)',
  },
  {
    type: 'scroll (element)',
    step: { type: 'scroll', selectors: [['.box']], x: 0, y: 50 },
    expectedContains: 'el.scrollTo(0, 50)',
  },
  {
    type: 'waitForElement (visible)',
    step: { type: 'waitForElement', selectors: [['#el']] },
    expectedContains: "waitFor({ state: 'visible' })",
  },
  {
    type: 'waitForElement (hidden)',
    step: { type: 'waitForElement', selectors: [['#el']], visible: false },
    expectedContains: "waitFor({ state: 'hidden' })",
  },
  {
    type: 'waitForExpression',
    step: { type: 'waitForExpression', expression: 'window.ready === true' },
    expectedContains: "waitForFunction('window.ready === true')",
  },
  {
    type: 'setViewport',
    step: {
      type: 'setViewport',
      width: 1280,
      height: 720,
      deviceScaleFactor: 1,
      isMobile: false,
      hasTouch: false,
      isLandscape: false,
    },
    expectedContains: null,
  },
  {
    type: 'emulateNetworkConditions',
    step: { type: 'emulateNetworkConditions', download: 1000, upload: 1000, latency: 100 },
    expectedContains: null,
  },
  {
    type: 'close',
    step: { type: 'close' },
    expectedContains: null,
  },
  {
    type: 'customStep',
    step: { type: 'customStep', name: 'myCustom', parameters: {} },
    expectedContains: null,
  },
];

describe('DevTools Recorder schema coverage', () => {
  for (const { type, step, expectedContains } of STEP_TYPE_SAMPLES) {
    it(`handles step type: ${type}`, () => {
      const input = JSON.stringify({ steps: [step] });
      const result = transformDevToolsRecording(input);

      expect(result).toContain('export async function execute(page)');

      if (expectedContains === null) {
        // The step should be skipped -- only the wrapper and hint should be present.
        const bodyLines = result
          .split('\n')
          .filter(
            (l) =>
              !l.startsWith('export') &&
              l !== '}' &&
              !l.includes('// TODO') &&
              !l.includes('// Examples') &&
              !l.includes('//   return'),
          );
        const nonEmpty = bodyLines.filter((l) => l.trim().length > 0);
        expect(nonEmpty).toHaveLength(0);
      } else {
        expect(result).toContain(expectedContains);
      }
    });
  }

  it('produces a comment for truly unknown step types', () => {
    const input = JSON.stringify({ steps: [{ type: 'futureNewStepType' }] });
    const result = transformDevToolsRecording(input);
    expect(result).toContain('// Unsupported step type: futureNewStepType');
  });

  it('all known @puppeteer/replay StepType values are covered', () => {
    const knownStepTypes = [
      'change',
      'click',
      'close',
      'customStep',
      'doubleClick',
      'emulateNetworkConditions',
      'hover',
      'keyDown',
      'keyUp',
      'navigate',
      'scroll',
      'setViewport',
      'waitForElement',
      'waitForExpression',
    ];

    const coveredTypes = new Set(
      STEP_TYPE_SAMPLES.map((s) => {
        const match = s.type.match(/^(\w+)/);
        return match ? match[1] : s.type;
      }),
    );

    for (const stepType of knownStepTypes) {
      expect(coveredTypes, `Missing coverage for step type: ${stepType}`).toContain(stepType);
    }
  });
});
