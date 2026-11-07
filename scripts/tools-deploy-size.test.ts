import assert from 'node:assert/strict';
import test from 'node:test';
import { formatDeploySize } from '../dev/tools/deploy.ts';

test('bundled pages distinguish source, body, embedded payloads, and separate responder scripts', () => {
  const report = formatDeploySize({
    originalSize: 75 * 1024,
    htmlSize: 60 * 1024,
    body: 'A'.repeat(256 * 1024),
    bundleBytes: new Map([['three-mf', 23 * 1024], ['mars', 173 * 1024]]),
    script: 'S'.repeat(59 * 1024),
  });
  assert.equal(
    report,
    'source 75.0 KB -> body 256.0 KB (HTML 60.0 KB + inline three-mf 23.0 KB + inline mars 173.0 KB; included in body) + separate responder script 59.0 KB',
  );
  assert.ok(!report.includes('% saved'));
});

test('unbundled HTML uses transformation labels rather than claiming compression savings', () => {
  assert.equal(formatDeploySize({ originalSize: 200, htmlSize: 100, body: 'A'.repeat(100) }), 'source 200 B -> body 100 B');
  assert.equal(formatDeploySize({ originalSize: 100, htmlSize: 200, body: 'A'.repeat(200) }), 'source 100 B -> body 200 B');
});

test('text responders and UTF-8 sizes are measured from the actual body', () => {
  assert.equal(formatDeploySize({ originalSize: 4, body: 'Ü✓' }), '5 B');
});
