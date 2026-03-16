const RETURN_HINT = [
  '',
  '  // TODO: Add a return statement with the content you want to track.',
  '  // Examples:',
  '  //   return await page.title();',
  '  //   return await page.content();',
  "  //   return await page.locator('.result').textContent();",
].join('\n');

interface DevToolsStep {
  type: string;
  url?: string;
  selectors?: (string | string[])[];
  offsetX?: number;
  offsetY?: number;
  value?: string;
  key?: string;
  x?: number;
  y?: number;
  button?: string;
  deviceType?: string;
  width?: number;
  height?: number;
  expression?: string;
  operator?: string;
  count?: number;
  properties?: Record<string, unknown>;
  visible?: boolean;
}

interface DevToolsRecording {
  title?: string;
  steps: DevToolsStep[];
}

/**
 * Pick the best selector from a DevTools selectors array.
 *
 * Priority:
 *  1. ARIA selectors (`aria/...`) -- map to Playwright semantic locators
 *  2. CSS selectors (plain strings without a prefix)
 *  3. XPath selectors (`xpath/...`)
 *  4. Text selectors (`text/...`)
 *  5. Pierce selectors (`pierce/...`) -- fall back to CSS
 *
 * Each entry in the array can be a string (direct selector) or a string[]
 * (ancestor chain for shadow DOM). We prefer simple string selectors.
 */
function pickSelector(selectors: (string | string[])[]): string {
  let bestAriaSelector: string | undefined;
  let bestCssSelector: string | undefined;
  let bestTextSelector: string | undefined;

  for (const s of selectors) {
    const flat = Array.isArray(s) ? s[s.length - 1] : s;
    if (!flat) {
      continue;
    }

    if (flat.startsWith('aria/') && !bestAriaSelector) {
      bestAriaSelector = flat;
    } else if (flat.startsWith('text/') && !bestTextSelector) {
      bestTextSelector = flat;
    } else if (!flat.startsWith('xpath/') && !flat.startsWith('pierce/') && !bestCssSelector) {
      bestCssSelector = flat;
    }
  }

  if (bestAriaSelector) {
    return ariaToPlaywright(bestAriaSelector);
  }
  if (bestCssSelector) {
    return `page.locator(${quote(bestCssSelector)})`;
  }
  if (bestTextSelector) {
    return `page.getByText(${quote(bestTextSelector.slice('text/'.length))})`;
  }

  // Fallback: use the first selector literally.
  const fallback = Array.isArray(selectors[0]) ? selectors[0][selectors[0].length - 1] : selectors[0];
  return `page.locator(${quote(fallback ?? '*')})`;
}

/**
 * Convert an `aria/Label[role="role"]` selector to a Playwright `getByRole` call.
 * Falls back to `getByLabel` when no role is specified.
 */
function ariaToPlaywright(ariaSelector: string): string {
  const body = ariaSelector.slice('aria/'.length);
  const roleMatch = body.match(/^(.*)\[role="(\w+)"\]$/);
  if (roleMatch) {
    const [, name, role] = roleMatch;
    return name?.trim()
      ? `page.getByRole(${quote(role)}, { name: ${quote(name.trim())} })`
      : `page.getByRole(${quote(role)})`;
  }
  return `page.getByLabel(${quote(body)})`;
}

function quote(value: string): string {
  if (value.includes("'") && !value.includes('"')) {
    return `"${value}"`;
  }
  return `'${value.replace(/'/g, "\\'")}'`;
}

function buttonOption(step: DevToolsStep): string {
  if (step.button === 'secondary') {
    return "{ button: 'right' }";
  }
  if (step.button === 'auxiliary') {
    return "{ button: 'middle' }";
  }
  return '';
}

function stepToPlaywright(step: DevToolsStep): string | null {
  switch (step.type) {
    case 'navigate':
      return `await page.goto(${quote(step.url ?? '')});`;

    case 'click': {
      if (!step.selectors?.length) {
        return null;
      }
      const loc = pickSelector(step.selectors);
      const opts = buttonOption(step);
      return opts ? `await ${loc}.click(${opts});` : `await ${loc}.click();`;
    }

    case 'doubleClick': {
      if (!step.selectors?.length) {
        return null;
      }
      const loc = pickSelector(step.selectors);
      return `await ${loc}.dblclick();`;
    }

    case 'hover': {
      if (!step.selectors?.length) {
        return null;
      }
      const loc = pickSelector(step.selectors);
      return `await ${loc}.hover();`;
    }

    case 'change': {
      if (!step.selectors?.length) {
        return null;
      }
      const loc = pickSelector(step.selectors);
      return `await ${loc}.fill(${quote(step.value ?? '')});`;
    }

    case 'keyDown':
      return `await page.keyboard.press(${quote(step.key ?? '')});`;

    case 'keyUp':
      // Playwright doesn't have a separate keyUp; keyDown + press covers most cases.
      // We skip keyUp steps to avoid redundant code.
      return null;

    case 'scroll': {
      if (step.selectors?.length) {
        const loc = pickSelector(step.selectors);
        return `await ${loc}.evaluate((el) => el.scrollTo(${step.x ?? 0}, ${step.y ?? 0}));`;
      }
      return `await page.evaluate(() => window.scrollTo(${step.x ?? 0}, ${step.y ?? 0}));`;
    }

    case 'waitForElement': {
      if (!step.selectors?.length) {
        return null;
      }
      const loc = pickSelector(step.selectors);
      const state = step.visible === false ? "'hidden'" : "'visible'";
      return `await ${loc}.waitFor({ state: ${state} });`;
    }

    case 'waitForExpression':
      return step.expression ? `await page.waitForFunction(${quote(step.expression)});` : null;

    // Steps we intentionally skip (no meaningful Playwright equivalent for page tracking).
    case 'setViewport':
    case 'emulateNetworkConditions':
    case 'close':
    case 'customStep':
      return null;

    default:
      return `// Unsupported step type: ${step.type}`;
  }
}

/**
 * Transforms a Chrome DevTools Recorder JSON recording into the Secutils
 * page tracker `execute(page, context)` format.
 */
export function transformDevToolsRecording(input: string): string {
  let recording: DevToolsRecording;
  try {
    recording = JSON.parse(input) as DevToolsRecording;
  } catch {
    throw new Error('Invalid JSON. Please paste a valid Chrome DevTools Recorder JSON export.');
  }

  if (!recording.steps || !Array.isArray(recording.steps)) {
    throw new Error('Invalid recording: missing "steps" array.');
  }

  const lines: string[] = [];
  for (const step of recording.steps) {
    const line = stepToPlaywright(step);
    if (line != null) {
      lines.push(`  ${line}`);
    }
  }

  const body = lines.join('\n');
  return `export async function execute(page) {\n${body}\n${RETURN_HINT}\n}`;
}
