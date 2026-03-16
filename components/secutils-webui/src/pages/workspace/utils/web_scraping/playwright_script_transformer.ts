const RETURN_HINT = [
  '',
  '  // TODO: Add a return statement with the content you want to track.',
  '  // Examples:',
  '  //   return await page.title();',
  '  //   return await page.content();',
  "  //   return await page.locator('.result').textContent();",
].join('\n');

const BOILERPLATE_PATTERNS = [
  // CommonJS require
  /^\s*const\s*\{[^}]*\}\s*=\s*require\s*\(\s*['"]playwright.*['"]\s*\)\s*;?\s*$/,
  // ESM import (playwright or @playwright/test)
  /^\s*import\s+.*\s+from\s+['"](?:@playwright\/test|playwright(?:-core)?)['"]\s*;?\s*$/,
  // import type
  /^\s*import\s+type\s+.*\s+from\s+['"](?:@playwright\/test|playwright(?:-core)?)['"]\s*;?\s*$/,
  // Single-line: browser.launch / chromium.launch / firefox.launch / webkit.launch
  /^\s*(?:const|let|var)\s+\w+\s*=\s*await\s+\w+\.launch\s*\([^)]*\)\s*;?\s*$/,
  // Single-line: browser.newContext
  /^\s*(?:const|let|var)\s+\w+\s*=\s*await\s+\w+\.newContext\s*\([^)]*\)\s*;?\s*$/,
  // Single-line: context.newPage
  /^\s*(?:const|let|var)\s+\w+\s*=\s*await\s+\w+\.newPage\s*\([^)]*\)\s*;?\s*$/,
  // context.close / browser.close
  /^\s*await\s+\w+\.close\s*\(\s*\)\s*;?\s*$/,
  // async IIFE open:  (async () => {
  /^\s*\(?\s*async\s*\(\s*\)\s*=>\s*\{?\s*$/,
  // IIFE close:  })();
  /^\s*\}\s*\)\s*\(\s*\)\s*;?\s*$/,
];

// Multi-line patterns that start boilerplate (e.g., chromium.launch({\n  headless: false\n});)
// These match when the call opens on one line and continues on subsequent lines.
// Matches either just `(` (empty args) or `({` (object literal args starting on same line)
const MULTILINE_BOILERPLATE_STARTS = [
  // browser.launch / chromium.launch / firefox.launch / webkit.launch - opening line only
  /^\s*(?:const|let|var)\s+\w+\s*=\s*await\s+\w+\.launch\s*\(\s*\{?\s*$/,
  // browser.newContext - opening line only
  /^\s*(?:const|let|var)\s+\w+\s*=\s*await\s+\w+\.newContext\s*\(\s*\{?\s*$/,
  // context.newPage - opening line only
  /^\s*(?:const|let|var)\s+\w+\s*=\s*await\s+\w+\.newPage\s*\(\s*\{?\s*$/,
];

// Lines that are assertions and should be stripped.
const ASSERTION_PATTERN = /^\s*await\s+expect\s*\(/;

// Matches the test() wrapper: test('name', async ({ page }) => {
const TEST_WRAPPER_OPEN =
  /^\s*test(?:\.describe)?\s*\(\s*['"`].*['"`]\s*,\s*async\s*\(\s*\{\s*page\s*\}\s*\)\s*=>\s*\{\s*$/;

/**
 * Transforms Playwright codegen output (test framework or library format)
 * into the Secutils page tracker `execute(page, context)` format.
 */
export function transformPlaywrightScript(input: string): string {
  const lines = input.split('\n');
  const pageLines: string[] = [];
  let insideTestWrapper = false;
  let testWrapperDepth = 0;
  let skipUntilSemicolon = false;

  for (const line of lines) {
    // Skip empty lines at the very beginning (before we've collected any real lines).
    if (pageLines.length === 0 && line.trim() === '') {
      continue;
    }

    // Check for test() wrapper open.
    if (TEST_WRAPPER_OPEN.test(line)) {
      insideTestWrapper = true;
      testWrapperDepth = 1;
      continue;
    }

    // When inside a test wrapper, track brace depth to find the real close.
    if (insideTestWrapper) {
      const openBraces = (line.match(/\{/g) || []).length;
      const closeBraces = (line.match(/\}/g) || []).length;
      testWrapperDepth += openBraces - closeBraces;
      if (testWrapperDepth <= 0) {
        insideTestWrapper = false;
        continue;
      }
    }

    // Check if we need to skip multi-line boilerplate (e.g., launch({...}))
    if (!skipUntilSemicolon) {
      if (MULTILINE_BOILERPLATE_STARTS.some((p) => p.test(line))) {
        skipUntilSemicolon = true;
        // Check if this line already has the semicolon (single-line case)
        if (line.trim().endsWith(';')) {
          skipUntilSemicolon = false;
        }
        continue;
      }
    }

    if (skipUntilSemicolon) {
      // Keep skipping until we find a line ending with semicolon
      if (line.trim().endsWith(';')) {
        skipUntilSemicolon = false;
      }
      continue;
    }

    // Skip known boilerplate lines.
    if (BOILERPLATE_PATTERNS.some((p) => p.test(line))) {
      continue;
    }

    // Skip assertion lines (await expect(...)).
    if (ASSERTION_PATTERN.test(line)) {
      continue;
    }

    pageLines.push(line);
  }

  // Trim trailing empty lines.
  while (pageLines.length > 0 && pageLines[pageLines.length - 1].trim() === '') {
    pageLines.pop();
  }

  // Normalize indentation: detect the minimum leading whitespace and strip it,
  // then re-indent with the standard 2-space offset for the function body.
  const nonEmptyLines = pageLines.filter((l) => l.trim().length > 0);
  const minIndent = nonEmptyLines.reduce((min, l) => {
    const match = l.match(/^(\s*)/);
    const len = match ? match[1].length : 0;
    return Math.min(min, len);
  }, Infinity);

  const normalizedLines = pageLines.map((l) => {
    if (l.trim().length === 0) {
      return '';
    }
    return '  ' + l.slice(isFinite(minIndent) ? minIndent : 0);
  });

  const body = normalizedLines.join('\n');
  return `export async function execute(page) {\n${body}\n${RETURN_HINT}\n}`;
}
